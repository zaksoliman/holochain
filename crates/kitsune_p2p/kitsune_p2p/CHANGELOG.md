# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

- BREAKING: Wire message `Call` no longer takes `from_agent`. [#1091](https://github.com/holochain/holochain/pull/1091)
## 0.0.14

## 0.0.13

## 0.0.12

- BREAKING: Return `ShardedGossipWire::Busy` if we are overloaded with incoming gossip. [\#1076](https://github.com/holochain/holochain/pull/1076)
  - This breaks the current network protocol and will not be compatible with other older versions of holochain (no manual action required).

## 0.0.11

## 0.0.10

- Check local agents for basis when doing a RPCMulti call. [\#1009](https://github.com/holochain/holochain/pull/1009).

## 0.0.9

- Fix rpc\_multi bug that caused all request to wait 3 seconds. [\#1009](https://github.com/holochain/holochain/pull/1009/)
- Fix to gossip’s round initiate. We were not timing out a round if there was no response to an initiate message. [\#1014](https://github.com/holochain/holochain/pull/1014).
- Make gossip only initiate with agents that have info that is not expired. [\#1014](https://github.com/holochain/holochain/pull/1014).

## 0.0.8

### Changed

- `query_gossip_agents`, `query_agent_info_signed`, and `query_agent_info_signed_near_basis` are now unified into a single `query_agents` call in `KitsuneP2pEvent`

## 0.0.7

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
