# `generate-backwards-compatibility-cases`
Generates "cases" for native interfaces.

Cases consist of both input and output of the native function, and are
serialized to files.

These cases are then deserialized and re-ran in the native crate, to verify
that the functions never change (in terms of their input -> output).
