/**
 * regex-to-railroad.ts
 * 将正则表达式转换为铁路图 SVG 字符串。
 * 使用 regexp-tree 解析正则为 AST，再递归转换为 railroad-diagrams 的图元素。
 */
import regexpTree from "regexp-tree";
import type {
  AstRegExp,
  Expression,
  Char,
  CharacterClass,
  ClassRange,
  Alternative,
  Disjunction,
  Group,
  Backreference,
  Repetition,
  Assertion,
} from "regexp-tree/ast";
import {
  Diagram,
  Sequence,
  Choice,
  Optional,
  OneOrMore,
  ZeroOrMore,
  Terminal,
  NonTerminal,
  Skip,
} from "railroad-diagrams";
import type { DiagramItem } from "railroad-diagrams";

/** 特殊字符的可读描述 */
const META_LABELS: Record<string, string> = {
  ".": "任意字符",
  "\\d": "数字 [0-9]",
  "\\D": "非数字",
  "\\w": "单词字符 [a-zA-Z0-9_]",
  "\\W": "非单词字符",
  "\\s": "空白字符",
  "\\S": "非空白字符",
  "\\b": "单词边界",
  "\\B": "非单词边界",
  "\\t": "制表符",
  "\\n": "换行符",
  "\\r": "回车符",
  "\\f": "换页符",
  "\\v": "垂直制表符",
  "\\0": "空字符",
};

/** 断言类型的可读描述 */
const ASSERTION_LABELS: Record<string, string> = {
  "^": "行首",
  "$": "行尾",
  "\\b": "单词边界",
  "\\B": "非单词边界",
};

/**
 * 将 regexp-tree AST 的 Char 节点转换为显示文本
 */
function charToLabel(node: Char): string {
  if (node.kind === "meta") {
    return META_LABELS[node.value] || node.value;
  }
  if (node.kind === "simple") {
    // 特殊处理 .（dot）
    if (node.value === ".") {
      return META_LABELS["."];
    }
    return node.value;
  }
  // control / hex / decimal / oct / unicode
  return node.value;
}

/**
 * 将 ClassRange 转换为可读字符串
 */
function classRangeToStr(node: ClassRange): string {
  return `${charToLabel(node.from)}-${charToLabel(node.to)}`;
}

/**
 * 将 CharacterClass 转换为可读标签
 */
function characterClassToLabel(node: CharacterClass): string {
  const prefix = node.negative ? "[^" : "[";
  const parts = node.expressions.map((expr) => {
    if (expr.type === "ClassRange") {
      return classRangeToStr(expr);
    }
    return charToLabel(expr as Char);
  });
  return `${prefix}${parts.join("")}]`;
}

/**
 * 收集 Disjunction 的所有分支（展平嵌套的 Disjunction）
 */
function collectDisjunctionBranches(node: Disjunction): Array<Expression | null> {
  const branches: Array<Expression | null> = [];
  if (node.left && node.left.type === "Disjunction") {
    branches.push(...collectDisjunctionBranches(node.left));
  } else {
    branches.push(node.left);
  }
  if (node.right && node.right.type === "Disjunction") {
    branches.push(...collectDisjunctionBranches(node.right));
  } else {
    branches.push(node.right);
  }
  return branches;
}

/**
 * 递归将 regexp-tree AST 节点转换为 railroad-diagrams 的图元素
 */
function astToRailroad(node: Expression | null): DiagramItem {
  if (!node) {
    return Skip();
  }

  switch (node.type) {
    case "Char": {
      const label = charToLabel(node);
      // 对元字符使用 NonTerminal（矩形框），普通字符用 Terminal（圆角框）
      if (node.kind === "meta" || (node.kind === "simple" && node.value === ".")) {
        return NonTerminal(label);
      }
      return Terminal(label);
    }

    case "CharacterClass": {
      const label = characterClassToLabel(node);
      return NonTerminal(label);
    }

    case "Alternative": {
      const alt = node as Alternative;
      if (alt.expressions.length === 0) {
        return Skip();
      }
      if (alt.expressions.length === 1) {
        return astToRailroad(alt.expressions[0]);
      }
      return Sequence(...alt.expressions.map((e) => astToRailroad(e)));
    }

    case "Disjunction": {
      const disj = node as Disjunction;
      const branches = collectDisjunctionBranches(disj);
      const items = branches.map((b) => astToRailroad(b));
      if (items.length === 0) {
        return Skip();
      }
      if (items.length === 1) {
        return items[0];
      }
      return Choice(0, ...items);
    }

    case "Group": {
      const group = node as Group;
      const inner = astToRailroad(group.expression);
      if (group.capturing) {
        // 捕获组：添加标签
        const label = group.name
          ? `#${group.number} "${group.name}"`
          : `#${group.number}`;
        // 用 Sequence 包裹，前后加注释标记
        return Sequence(
          NonTerminal(`( ${label}`),
          inner,
          NonTerminal(")")
        );
      }
      // 非捕获组：直接返回内部
      return inner;
    }

    case "Backreference": {
      const backref = node as Backreference;
      if (backref.kind === "name") {
        return NonTerminal(`\\k<${backref.reference}>`);
      }
      return NonTerminal(`\\${backref.reference}`);
    }

    case "Repetition": {
      const rep = node as Repetition;
      const inner = astToRailroad(rep.expression);
      const q = rep.quantifier;
      const greedyLabel = q.greedy ? "" : " (lazy)";

      if (q.kind === "*") {
        // ZeroOrMore
        const item = ZeroOrMore(inner);
        if (!q.greedy) {
          return Sequence(item, Terminal(greedyLabel.trim()));
        }
        return item;
      }
      if (q.kind === "+") {
        // OneOrMore
        const item = OneOrMore(inner);
        if (!q.greedy) {
          return Sequence(item, Terminal(greedyLabel.trim()));
        }
        return item;
      }
      if (q.kind === "?") {
        // Optional
        return Optional(inner);
      }
      // Range quantifier {n,m}
      if (q.kind === "Range") {
        let label: string;
        if (q.to === undefined) {
          // {n,} -- n or more
          label = `{${q.from},}`;
        } else if (q.from === q.to) {
          // {n} -- exactly n
          label = `{${q.from}}`;
        } else {
          // {n,m}
          label = `{${q.from},${q.to}}`;
        }
        if (!q.greedy) {
          label += "?";
        }
        // 用 OneOrMore + Comment 的方式表示次数
        return Sequence(inner, NonTerminal(label));
      }

      return inner;
    }

    case "Assertion": {
      const assertion = node as Assertion;
      if (assertion.kind === "Lookahead" || assertion.kind === "Lookbehind") {
        const prefix =
          assertion.kind === "Lookahead"
            ? assertion.negative
              ? "(?!"
              : "(?="
            : assertion.negative
              ? "(?<!"
              : "(?<=";
        const inner = astToRailroad(assertion.assertion);
        return Sequence(NonTerminal(prefix), inner, NonTerminal(")"));
      }
      // Simple assertion: ^, $, \b, \B
      const label = ASSERTION_LABELS[assertion.kind] || assertion.kind;
      return NonTerminal(label);
    }

    default:
      return Terminal(String((node as Expression).type || "?"));
  }
}

/**
 * 内联 CSS 样式，用于 SVG 在没有外部样式表时也能正确渲染
 */
const INLINE_STYLE = `
  svg.railroad-diagram {
    border: none;
  }
  svg.railroad-diagram path {
    stroke-width: 3;
    stroke: var(--lc-accent, #3b82f6);
    fill: rgba(0,0,0,0);
  }
  svg.railroad-diagram text {
    font: bold 14px monospace;
    text-anchor: middle;
    fill: var(--lc-text, #e0e0e0);
  }
  svg.railroad-diagram rect {
    stroke-width: 3;
    stroke: var(--lc-accent, #3b82f6);
    fill: var(--lc-surface-1, #1e1e2e);
  }
  svg.railroad-diagram .non-terminal rect {
    stroke: var(--lc-accent-light, #60a5fa);
  }
`;

/**
 * 将正则表达式字符串转换为铁路图 SVG 字符串。
 *
 * @param pattern 正则表达式字符串（不含定界符 /.../ ）
 * @returns SVG 字符串；若解析失败则返回以 "Error:" 开头的错误消息
 */
export function regexToSvg(pattern: string): string {
  if (!pattern || !pattern.trim()) {
    return "";
  }

  try {
    const ast: AstRegExp = regexpTree.parse(`/${pattern}/`);
    const body = ast.body;

    if (!body) {
      return "";
    }

    const railroadElement = astToRailroad(body);
    const diagram = Diagram(railroadElement);
    const svgStr = diagram.toString();

    // 注入内联样式
    const styleTag = `<style>${INLINE_STYLE}</style>`;
    // 在 <svg 后的第一个 > 之后注入 style
    const injected = svgStr.replace(/>/, `>${styleTag}`);

    return injected;
  } catch (err: unknown) {
    const message =
      err instanceof Error ? err.message : String(err);
    return `Error: ${message}`;
  }
}
