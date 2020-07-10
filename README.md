# Holochain

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)
License: [![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

This repository contains most of the core Holochain libraries and binaries. This implementation of Holochain is focused on correctness, data integrity, and performance. It is intended to serve as a full replacement to the previous holochain-rust version (a.k.a holochain-redux) and although this implementation is not yet complete, it already surpasses prior versions in features and performance.

**Code Status:** This code is pre-alpha, not ready for use in production. The code is NOT guaranteed to be stable nor secure. You should also expect frequent changes to the Holochain API and significant modifications to the HDK until Beta release.

## Overview

 - [Holochain Implementation Architectural Overview](./docs/TODO.md)
 - [Holochain's Formal State Model](./docs/formalization.md)
 - [Holochain Workflow Diagrams](./docs/TODO.md)
 - [Major changes from Redux to Workflow model](./docs/major_changes.md)

## Application Developer

This version of Holochain has simplified direct calls from your applications compiled to wasm to Holochain's system functions. You can either use our HDK (which is intended to simplify these calls with a Holochain DSL that handles code generation for you via Rust macros), or bypass the HDK and write direct system calls yourself.
 - [Docs for making direct API calls](./crates/hkd/README.md)
 - [Working docs for coming HDK changes](./docs/HDK_planning.md)

## Core Developer

[Core Developer Setup](./docs/TODO.md)

## Contribute
Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

* Connect with us on our [developer forum](https://forum.holochain.org)

## License
 [![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Copyright (C) 2019 - 2020, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (CAL-1.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
