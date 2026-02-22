declare module "railroad-diagrams" {
  interface DiagramItem {
    width: number;
    up: number;
    down: number;
    needsSpace?: boolean;
    format(x: number, y: number, width: number): DiagramItem;
    addTo(parent: DiagramItem | HTMLElement): DiagramItem | SVGElement;
    toString(): string;
    toSVG(): SVGElement;
  }

  interface DiagramStatic {
    (
      ...items: Array<string | DiagramItem>
    ): DiagramItem;
    new (
      items: Array<string | DiagramItem>
    ): DiagramItem;
    VERTICAL_SEPARATION: number;
    ARC_RADIUS: number;
    DIAGRAM_CLASS: string;
    STROKE_ODD_PIXEL_LENGTH: boolean;
    INTERNAL_ALIGNMENT: string;
  }

  export const Diagram: DiagramStatic;
  export const ComplexDiagram: DiagramStatic;

  export function Sequence(
    ...items: Array<string | DiagramItem>
  ): DiagramItem;
  export function Choice(
    normal: number,
    ...items: Array<string | DiagramItem>
  ): DiagramItem;
  export function Optional(
    item: string | DiagramItem,
    skip?: "skip"
  ): DiagramItem;
  export function OneOrMore(
    item: string | DiagramItem,
    rep?: string | DiagramItem
  ): DiagramItem;
  export function ZeroOrMore(
    item: string | DiagramItem,
    rep?: string | DiagramItem,
    skip?: "skip"
  ): DiagramItem;
  export function Terminal(text: string): DiagramItem;
  export function NonTerminal(text: string): DiagramItem;
  export function Comment(text: string): DiagramItem;
  export function Skip(): DiagramItem;
}
