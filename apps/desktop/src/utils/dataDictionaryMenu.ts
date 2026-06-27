export type DictionaryMenuCommand = "replace" | "fields" | "rebuild" | "rename" | "delete";

export interface DictionaryMenuHandlers {
  replace: () => void | Promise<void>;
  fields: () => void | Promise<void>;
  rebuild: () => void | Promise<void>;
  rename: () => void | Promise<void>;
  remove: () => void | Promise<void>;
}

export type DictionaryMenuScheduler = (task: () => void) => void;

function defaultSchedule(task: () => void): void {
  setTimeout(task, 0);
}

export function isDictionaryMenuCommand(command: unknown): command is DictionaryMenuCommand {
  return (
    command === "replace" ||
    command === "fields" ||
    command === "rebuild" ||
    command === "rename" ||
    command === "delete"
  );
}

export function dispatchDictionaryMenuCommand(
  command: unknown,
  handlers: DictionaryMenuHandlers,
  schedule: DictionaryMenuScheduler = defaultSchedule,
): boolean {
  if (!isDictionaryMenuCommand(command)) return false;

  const run = () => {
    if (command === "replace") {
      void handlers.replace();
      return;
    }
    if (command === "fields") {
      void handlers.fields();
      return;
    }
    if (command === "rebuild") {
      void handlers.rebuild();
      return;
    }
    if (command === "rename") {
      void handlers.rename();
      return;
    }
    void handlers.remove();
  };

  if (command === "replace" || command === "fields") {
    schedule(run);
    return true;
  }

  run();
  return true;
}
