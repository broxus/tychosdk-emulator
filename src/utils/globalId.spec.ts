import { beginCell, Cell, Dictionary } from "@ton/core";
import { defaultGlobalId, defaultConfig } from "../config/defaultConfig";
import { CellRef, GLOBAL_ID_IDX, getGlobalId } from "./globalId";

describe("getGlobalId", () => {
  it("should properly extract global id from the default config", () => {
    expect(getGlobalId(Cell.fromBase64(defaultConfig))).toBe(defaultGlobalId);
  });

  it("should return undefined when no global id is set", () => {
    const configRoot = Cell.fromBase64(defaultConfig);
    const configDict = Dictionary.loadDirect(
      Dictionary.Keys.Int(32),
      CellRef,
      configRoot
    );
    configDict.delete(GLOBAL_ID_IDX);

    const newConfigRoot = beginCell().storeDictDirect(configDict).endCell();
    expect(getGlobalId(newConfigRoot)).toBeUndefined();
  });
});
