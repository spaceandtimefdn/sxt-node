# `proof-of-sql-unversioned`
Macros and utilities for proof-of-sql that are written in a version-agnostic way.

Some components of the chain need to upgrade their proof-of-sql version often, while others need to upgrade very conservatively (native APIs). This crate aims to reduce repeated code between these components.
