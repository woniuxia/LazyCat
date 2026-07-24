import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./VaultEntryDialog.vue", import.meta.url), "utf8");
const serverFields = source.slice(
  source.indexOf("<!-- Server fields -->"),
  source.indexOf("<!-- Database fields -->"),
);
const saveFlow = source.slice(source.indexOf("async function onSave"));

describe("VaultEntryDialog server port", () => {
  it("renders a bounded server port input", () => {
    expect(serverFields).toContain('label="端口"');
    expect(serverFields).toContain('v-model="form.port"');
    expect(serverFields).toContain(':min="1"');
    expect(serverFields).toContain(':max="65535"');
  });

  it("defaults server ports to 22 for new and legacy entries", () => {
    const editPortFallback = source.slice(
      source.indexOf("form.port = typeof f.port"),
      source.indexOf("form.dbName", source.indexOf("form.port = typeof f.port")),
    );
    expect(source).toContain("const SERVER_DEFAULT_PORT = 22;");
    expect(source).toContain('newCat === "server"');
    expect(source).toContain("form.port = SERVER_DEFAULT_PORT;");
    expect(editPortFallback).toContain('form.category === "server"');
    expect(editPortFallback).toContain("? SERVER_DEFAULT_PORT");
  });

  it("submits the server port", () => {
    const serverSave = saveFlow.slice(
      saveFlow.indexOf('form.category === "server"'),
      saveFlow.indexOf('form.category === "database"'),
    );
    expect(serverSave).toContain("payload.port = form.port;");
  });
});
