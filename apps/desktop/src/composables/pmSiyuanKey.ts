import type { InjectionKey } from "vue";
import type { usePmSiyuan } from "./usePmSiyuan";

export const PM_SIYUAN_KEY: InjectionKey<ReturnType<typeof usePmSiyuan>> =
  Symbol("pmSiyuan");
