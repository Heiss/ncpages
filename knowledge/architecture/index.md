# Architecture

How the system is put together. The *why* behind each choice lives in
[decisions](../decisions/).

* [Pipeline](pipeline.md) - the ten steps from trigger to report, and why the order is fixed
* [Scheduler state machine](state-machine.md) - debounce, hard deadline, queueing, persistence, startup reconcile
* [Topology](topology.md) - three containers, the bridge network, volumes, and the build tree
* [Security model](security-model.md) - a build is code execution; content/code separation, sandbox, credential isolation
* [Delivery](delivery.md) - atomic publish, releases and rollback, caching headers
* [Build environment](build-environment.md) - how the builder image gets to run without a network
