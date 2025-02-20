# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]
- Some databases can handle corruption by wiping the db file and starting again. [#1039](https://github.com/holochain/holochain/pull/1039).

## 0.0.16

## 0.0.15

## 0.0.14

- BREAKING CHANGE. Source chain `query` will now return results in header sequence order ascending.

## 0.0.13

## 0.0.12

## 0.0.11

## 0.0.10

## 0.0.9

- Fixed a bug when creating an entry with `ChainTopOrdering::Relaxed`, in which the header was created and stored in the Source Chain, but the actual entry was not.
- Geneis ops will no longer run validation for the authored node and only genesis self check will run. [\#995](https://github.com/holochain/holochain/pull/995)

## 0.0.8

## 0.0.7

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
