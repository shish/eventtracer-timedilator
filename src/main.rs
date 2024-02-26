use std::borrow::Cow;
use std::{collections::HashMap, fs, time::SystemTime};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use strum::AsRefStr;
use strum::EnumString;

use clap::Parser;

type TS = i64;

// copy-pasted from
// https://docs.rs/tracing-chrometrace/latest/src/tracing_chrometrace/lib.rs.html#84-120
#[derive(Debug, Copy, Clone, Default, EnumString, AsRefStr, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    #[serde(rename = "B")]
    DurationBegin,
    #[serde(rename = "E")]
    DurationEnd,
    #[serde(rename = "X")]
    Complete,
    #[default]
    #[serde(rename = "i")]
    Instant,
    #[serde(rename = "C")]
    Counter,
    #[serde(rename = "b")]
    AsyncStart,
    #[serde(rename = "n")]
    AsyncInstant,
    #[serde(rename = "e")]
    AsyncEnd,
    #[serde(rename = "s")]
    FlowStart,
    #[serde(rename = "t")]
    FlowStep,
    #[serde(rename = "f")]
    FlowEnd,
    #[serde(rename = "p")]
    Sample,
    #[serde(rename = "N")]
    ObjectCreated,
    #[serde(rename = "O")]
    ObjectSnapshot,
    #[serde(rename = "D")]
    ObjectDestroyed,
    #[serde(rename = "M")]
    Metadata,
    #[serde(rename = "V")]
    MemoryDumpGlobal,
    #[serde(rename = "v")]
    MemoryDumpProcess,
    #[serde(rename = "R")]
    Mark,
    #[serde(rename = "c")]
    ClockSync,
    #[serde(rename = "(")]
    ContextBegin,
    #[serde(rename = ")")]
    ContextEnd,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChromeEvent {
    #[serde(default = "SystemTime::now")]
    #[serde(skip)]
    #[allow(unused)]
    start: SystemTime,
    #[serde(default)]
    pub name: Cow<'static, str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub cat: Cow<'static, str>,
    pub ph: EventType,
    pub ts: TS,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dur: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts: Option<f64>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub id: Cow<'static, str>,
    pub pid: u64,
    pub tid: u64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, serde_json::Value>,
}

#[derive(Parser)]
struct Cli {
    #[clap()]
    input: PathBuf,
    #[clap()]
    output: PathBuf,
}

#[derive(Copy, Clone)]
enum ThreadState {
    Running(u64),
    Sleeping(TS),
}

#[derive(Copy, Clone)]
struct ThreadStat {
    gap_total: TS,
    state: ThreadState,
}
fn main() -> Result<()> {
    let args = Cli::parse();

    // keep popping the final character from data until we reach
    // the end of an object, then close it properly. This accounts
    // for files which end as "}" or "}," or "},\n" (ie, files
    // designed to be appended-to) instead of the official "}]"
    let mut data = fs::read_to_string(args.input)?;
    while data.pop() != Some('}') {}
    data.push_str("}]");

    // parse the json into a vector of ChromeEvents
    let mut events: Vec<ChromeEvent> = serde_json::from_str(&data)?;
    if events.is_empty() {
        return Ok(());
    }
    let all_start = events[0].ts;

    // build a HashMap of the unique thread ids to the first timestamp
    let mut threads: HashMap<u64, ThreadStat> = HashMap::new();

    for event in &mut events {
        event.ts -= all_start;

        threads.entry(event.tid).or_insert(ThreadStat {
            gap_total: 0,
            state: ThreadState::Sleeping(0),
        });

        let thread = threads.get_mut(&event.tid).unwrap();
        match (thread.state, event.ph) {
            // if we're sleeping and we get a begin, set the thread to running
            (ThreadState::Sleeping(slept_at), EventType::DurationBegin) => {
                thread.gap_total += event.ts - slept_at;
                thread.state = ThreadState::Running(1);
            }
            // if we're already running and we get a begin, increment the depth
            (ThreadState::Running(depth), EventType::DurationBegin) => {
                thread.state = ThreadState::Running(depth + 1);
            }
            // if we're running and we get an end, decrement the depth,
            // and if we're at depth 0, set the thread to sleeping
            (ThreadState::Running(depth), EventType::DurationEnd) => {
                if depth == 1 {
                    thread.state = ThreadState::Sleeping(event.ts);
                } else {
                    thread.state = ThreadState::Running(depth - 1);
                }
            }
            // if we're running and we get any other event, just adjust the event time
            (ThreadState::Running(_), _) => {}
            // if we're sleeping and we get anything other than begin, something went wrong
            (ThreadState::Sleeping(_), _) => {
                panic!("Got an event for a sleeping thread");
            }
        }
        event.ts -= thread.gap_total;
    }

    // write the modified events to a new file
    let modified_data = serde_json::to_string(&events)?;
    fs::write(args.output, modified_data)?;

    Ok(())
}
