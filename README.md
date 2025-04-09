# Tycho Emulator

A Tycho executor for [TON Sandbox](https://github.com/ton-org/sandbox).

## Installation

Requires node 16 or higher.

```bash
yarn add @tychosdk/emulator @ton/sandbox @ton/ton @ton/core @ton/crypto
```

or

```bash
npm i @tychosdk/emulator @ton/sandbox @ton/ton @ton/core @ton/crypto
```

## Usage

```typescript
import { Blockchain } from "@ton/sandbox";
import { TychoExecutor } from "@tychosdk/emulator";

const executor = await TychoExecutor.create();
const blockchain = await Blockchain.create({
  executor,
  config: TychoExecutor.defaultConfig,
});

const version = executor.getVersion();
console.log("Version:", version);
```

## Development

### @tychosdk/emulator

To install dependencies:

```bash
yarn install
```

To build wasm:

```bash
yarn wasm:build
```

To run tests:

```bash
yarn test
```

To publish:

```bash
yarn build
yarn publish --access public
```

## Contributing

We welcome contributions to the project! If you notice any issues or errors,
feel free to open an issue or submit a pull request.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)
  or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT)
  or <https://opensource.org/licenses/MIT>)

at your option.
