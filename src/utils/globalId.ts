import { Cell, Dictionary, DictionaryValue } from "@ton/core";

const CellRef: DictionaryValue<Cell> = {
  serialize: (src, builder) => {
    builder.storeRef(src);
  },
  parse: (src) => src.loadRef(),
};

const GLOBAL_ID_IDX = 19;

export function getGlocalId(configRoot: Cell): number | undefined {
  const configDict = Dictionary.loadDirect(
    Dictionary.Keys.Int(32),
    CellRef,
    configRoot
  );

  const globalIdValue = configDict.get(GLOBAL_ID_IDX);
  return globalIdValue?.asSlice().loadUint(32);
}
