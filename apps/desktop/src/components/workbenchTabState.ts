export type JsonWorkbenchTab = "process" | "schema" | "array-filter";
export type DataConvertTab = "csv" | "java-bean" | "config";

// 只保留当前应用运行期状态；应用重启后模块会重新使用首个页签。
export const workbenchTabState: {
  json: JsonWorkbenchTab;
  dataConvert: DataConvertTab;
} = {
  json: "process",
  dataConvert: "csv",
};
