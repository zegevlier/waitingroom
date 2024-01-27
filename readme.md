# Waiting room

This is a waiting room implementation as part of my Bachelor's thesis in Computing Science. It is currently far from done, and I would not recommend using it in its current state as changes are still being made fast and often.

## Current to-do list
This contains some things that will happen soon, and some things which might not happen at all. It is (roughly) in order of what I will do first, and some tasks are going to be split in more, smaller tasks.
- [ ] Add proper logging and tracing to `waitingroom-http`
- [ ] Find out the cause + fix the bug causing too many people to be on the site at once
- [ ] Write documentation for `Pass`
- [ ] Write documentation for `Ticket`
- [ ] Write documentation for `WaitingroomError`
- [ ] Make fake user managing tool (TUI based, probably)
- [x] Ensure consistent usage of `waitingroom` vs `waiting_room`
- [x] Run spell-checker on everything
- [ ] Document metrics and move them to `waitingroom-metrics`
- [ ] Set up docker container images to make running prometheus and grafana for the dashboard easier
- [ ] Add message queue to waiting room and fix functions being called non-atomically
- [ ] Re-make parts of `waitingroom-http` to make the code more self-documenting and overall better
- [ ] Add cross-node message passing to `waitingroom-http` to prepare for real implementation
- [ ] Write QPID-based waiting room (The part that's actually the thesis lol)
- [ ] Write benchmarking skeleton and run benchmarks
- [ ] Write testing skeleton, write tests and run time