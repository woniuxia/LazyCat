declare module "frappe-gantt" {
  export interface Task {
    id: string;
    name: string;
    start: string;
    end: string;
    progress?: number;
    dependencies?: string;
    custom_class?: string;
    description?: string;
  }

  export interface GanttPopupContext {
    task: Task;
    chart: Gantt;
    get_title: () => HTMLElement;
    set_title: (title: string) => void;
    get_subtitle: () => HTMLElement;
    set_subtitle: (subtitle: string) => void;
    get_details: () => HTMLElement;
    set_details: (details: string) => void;
    add_action: (
      html: string | ((task: Task) => string),
      func: (task: Task, chart: Gantt, event: MouseEvent) => void,
    ) => void;
  }

  export interface GanttOptions {
    header_height?: number;
    column_width?: number;
    step?: number;
    view_modes?: string[];
    bar_height?: number;
    bar_corner_radius?: number;
    arrow_curve?: number;
    padding?: number;
    view_mode?: string;
    date_format?: string;
    language?: string;
    readonly?: boolean;
    popup?: false | ((context: GanttPopupContext) => string | false | void);
    popup_on?: "click" | "hover";
    on_click?: (task: Task) => void;
    on_double_click?: (task: Task) => void;
    on_date_change?: (task: Task, start: Date, end: Date) => void;
    on_progress_change?: (task: Task, progress: number) => void;
    on_view_change?: (mode: string) => void;
  }

  class Gantt {
    constructor(wrapper: string | HTMLElement, tasks: Task[], options?: GanttOptions);
    change_view_mode(mode: string, maintain_pos?: boolean): void;
    refresh(tasks: Task[]): void;
    hide_popup(): void;
  }

  export default Gantt;
}
