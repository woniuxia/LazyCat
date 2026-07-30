import { beforeEach, describe, expect, it } from "vitest";

import { useActionCenterNavigation } from "./useActionCenterNavigation";

describe("useActionCenterNavigation", () => {
  const navigation = useActionCenterNavigation();

  beforeEach(() => {
    const current = navigation.pendingTarget.value;
    if (current) navigation.consume(current);
  });

  it("stores combination and run navigation until the panel consumes it", () => {
    navigation.requestCombination(7);
    expect(navigation.pendingTarget.value).toEqual({ kind: "combination", combinationId: 7 });
    const combination = navigation.pendingTarget.value!;
    navigation.consume(combination);
    expect(navigation.pendingTarget.value).toBeNull();

    navigation.requestRun(" run-9 ");
    expect(navigation.pendingTarget.value).toEqual({ kind: "run", runId: "run-9" });
  });

  it("rejects invalid identifiers and does not consume a newer target", () => {
    navigation.requestCombination(0);
    navigation.requestRun("   ");
    expect(navigation.pendingTarget.value).toBeNull();

    navigation.requestRun("run-old");
    const old = navigation.pendingTarget.value!;
    navigation.requestRun("run-new");
    navigation.consume(old);
    expect(navigation.pendingTarget.value).toEqual({ kind: "run", runId: "run-new" });
  });
});
