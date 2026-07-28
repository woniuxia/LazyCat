import { beforeEach, describe, expect, it } from "vitest";

import { useActionDispatchIntent } from "./useActionDispatchIntent";

const firstIntent = {
  dispatchId: "dispatch-1",
  actionType: "release_package.run",
  targetToolId: "release-package",
  targetId: "42",
};

describe("useActionDispatchIntent", () => {
  beforeEach(() => {
    useActionDispatchIntent().pendingIntent.value = null;
  });

  it("only consumes an intent from its target tool", () => {
    const center = useActionDispatchIntent();
    center.setPendingIntent(firstIntent);

    expect(center.consumePendingIntent("todo")).toBeNull();
    expect(center.consumePendingIntent("release-package")?.dispatchId).toBe("dispatch-1");
    expect(center.consumePendingIntent("release-package")).toBeNull();
  });

  it("replaces an older unconsumed intent with the latest request", () => {
    const center = useActionDispatchIntent();
    center.setPendingIntent(firstIntent);
    center.setPendingIntent({
      ...firstIntent,
      dispatchId: "dispatch-2",
      targetId: "43",
    });

    expect(center.consumePendingIntent("release-package")).toMatchObject({
      dispatchId: "dispatch-2",
      targetId: "43",
    });
  });
});
