EventTracer Time Dilator
========================
Removes empty gaps in chrome event traces

Why
---
Because my webapp records event traces in RAM with every request, and dumps them to disk only if a request takes longer than a second. This means that I typically have one 1-2 second long trace, then a couple of hours of empty space, and then another 1-2 second trace. This is a pain to navigate beacuse there's so much empty space.

Before: 3 seconds of data spread across 2 hours
![before](./.github/before.png)

After: 3 seconds of data spread across 3 seconds
![after](./.github/after.png)


How do I use it
---------------
```
cargo run -- input.json output.json
```

```
cargo run -- input.json   # will write to input.td.json
```
