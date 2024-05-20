# Waiting room

This is a waiting room implementation as part of my Bachelor's thesis in Computing Science. It is currently far from done, and I would not recommend using it in its current state as changes are still being made fast and often.

## Current to-do list
This contains some things that should happen soon™. These are all things I intend to do before the end of my thesis, and they're roughly in order of priority. I will split them up into smaller tasks as I go along.
- [ ] Finish writing the distributed waiting room simulation:
    - [x] Make the letting out of users trigger when it hasn't happened yet and a new root is selected.
    - [x] Implement Round-Robin Probe Target Selection.
    - [x] Better document the recently added code
    - [x] Detect node failures automatically (using simplified SWIM)
    - [ ] Add recovery from node failures (TODO: Split)
- [ ] Write automated deterministic testing system (TODO: Split)
- [ ] Write benchmarking skeleton and run benchmarks (TODO: Split)

The following are things that I will likely not do before finishing my thesis, as I am focussing on a simulation only for now. They are here so I don't forget about them. I do intend to do them at some point, but only after my thesis is done.
- [ ] Document metrics and move them out of to `waitingroom-metrics` crate
- [ ] Move settings parsing with foundation out of `waitingroom-core` so the waiting room can be used without foundation 
- [ ] Set up docker container images to make running prometheus and grafana for the dashboard easier
- [ ] Re-make parts (most) of `waitingroom-http` to make the code more self-documenting and overall better
- [ ] Add cross-node message passing to `waitingroom-http` to make distributed implementation work
- [ ] Make `waitingroom-http` work with the distributed waiting room
- [ ] Write a proper readme with usage instructions etc.
