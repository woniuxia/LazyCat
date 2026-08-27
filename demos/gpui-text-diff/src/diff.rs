//! 行级文本 diff：小输入走标准 LCS 动态规划；超大输入退化为
//! 前后缀对齐（快速模式），避免 O(n*m) 内存爆炸。

/// 超过该行数（任一侧）时放弃 LCS，使用快速模式。
const MAX_LCS_LINES: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// 两侧相同
    Equal,
    /// 仅右侧有（新增）
    Insert,
    /// 仅左侧有（删除）
    Delete,
}

#[derive(Debug, Clone)]
pub struct DiffRow {
    pub kind: RowKind,
    /// 左侧行号（从 1 开始），Equal/Delete 有值
    pub a_no: Option<usize>,
    /// 右侧行号（从 1 开始），Equal/Insert 有值
    pub b_no: Option<usize>,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffStats {
    pub equal: usize,
    pub inserted: usize,
    pub deleted: usize,
}

impl DiffStats {
    pub fn has_changes(&self) -> bool {
        self.inserted > 0 || self.deleted > 0
    }
}

/// 拆分为逻辑行：按 `\n` 切分，容忍 `\r\n`；
/// 结尾的单个换行不产生空尾行。
fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    text.lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect()
}

/// 计算行级差异。返回 (差异行序列, 是否启用了快速模式)。
pub fn compute_rows(a_text: &str, b_text: &str) -> (Vec<DiffRow>, bool) {
    let a = split_lines(a_text);
    let b = split_lines(b_text);
    if a.len() > MAX_LCS_LINES || b.len() > MAX_LCS_LINES {
        (fast_rows(&a, &b), true)
    } else {
        (lcs_rows(&a, &b), false)
    }
}

/// 标准 LCS 动态规划 + 回溯。
fn lcs_rows(a: &[String], b: &[String]) -> Vec<DiffRow> {
    let n = a.len();
    let m = b.len();
    // dp[i][j] = a[i..] 与 b[j..] 的 LCS 长度
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut rows = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    let (mut no_a, mut no_b) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            no_a += 1;
            no_b += 1;
            rows.push(DiffRow {
                kind: RowKind::Equal,
                a_no: Some(no_a),
                b_no: Some(no_b),
                text: a[i].clone(),
            });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            no_a += 1;
            rows.push(DiffRow {
                kind: RowKind::Delete,
                a_no: Some(no_a),
                b_no: None,
                text: a[i].clone(),
            });
            i += 1;
        } else {
            no_b += 1;
            rows.push(DiffRow {
                kind: RowKind::Insert,
                a_no: None,
                b_no: Some(no_b),
                text: b[j].clone(),
            });
            j += 1;
        }
    }
    while i < n {
        no_a += 1;
        rows.push(DiffRow {
            kind: RowKind::Delete,
            a_no: Some(no_a),
            b_no: None,
            text: a[i].clone(),
        });
        i += 1;
    }
    while j < m {
        no_b += 1;
        rows.push(DiffRow {
            kind: RowKind::Insert,
            a_no: None,
            b_no: Some(no_b),
            text: b[j].clone(),
        });
        j += 1;
    }
    rows
}

/// 快速模式：剥离公共前后缀，中间整体替换。
fn fast_rows(a: &[String], b: &[String]) -> Vec<DiffRow> {
    // 公共前缀
    let mut pre = 0usize;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    // 公共后缀（不与前缀重叠）
    let mut suf = 0usize;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf]
    {
        suf += 1;
    }

    let mut rows = Vec::new();
    let mut push_equal = |range: std::ops::Range<usize>, src: &[String]| {
        for (k, idx) in range.enumerate() {
            let _ = k;
            rows.push(DiffRow {
                kind: RowKind::Equal,
                a_no: Some(idx + 1),
                b_no: Some(idx + 1),
                text: src[idx].clone(),
            });
        }
    };

    push_equal(0..pre, a);
    for (idx, line) in a.iter().enumerate().take(a.len() - suf).skip(pre) {
        rows.push(DiffRow {
            kind: RowKind::Delete,
            a_no: Some(idx + 1),
            b_no: None,
            text: line.clone(),
        });
    }
    let b_tail_start = pre; // 右侧前缀同样为 pre
    for (idx, line) in b.iter().enumerate().take(b.len() - suf).skip(b_tail_start) {
        rows.push(DiffRow {
            kind: RowKind::Insert,
            a_no: None,
            b_no: Some(idx + 1),
            text: line.clone(),
        });
    }
    // 尾部等号段：两侧长度不同，分别编号
    let a_start = a.len() - suf;
    let b_start = b.len() - suf;
    for k in 0..suf {
        rows.push(DiffRow {
            kind: RowKind::Equal,
            a_no: Some(a_start + k + 1),
            b_no: Some(b_start + k + 1),
            text: a[a_start + k].clone(),
        });
    }
    rows
}

pub fn summarize(rows: &[DiffRow]) -> DiffStats {
    let mut s = DiffStats::default();
    for r in rows {
        match r.kind {
            RowKind::Equal => s.equal += 1,
            RowKind::Insert => s.inserted += 1,
            RowKind::Delete => s.deleted += 1,
        }
    }
    s
}

// ---------- 并排（双栏）视图 ----------

/// 并排对比中的半行：一侧的具体内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HalfLine {
    pub kind: RowKind,
    /// 行号（从 1 开始）
    pub no: usize,
    pub text: String,
}

/// 并排对比的一行：左（A）右（B）各一个半行，
/// `None` 表示该侧为空白占位（对面的行是删除/新增）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairRow {
    pub left: Option<HalfLine>,
    pub right: Option<HalfLine>,
}

fn half_from(row: &DiffRow) -> HalfLine {
    HalfLine {
        kind: row.kind,
        no: row.a_no.or(row.b_no).unwrap_or_default(),
        text: row.text.clone(),
    }
}

/// 把线性 op 序列折叠成并排对齐的行：
/// 连续的非 Equal 段视为一个修改块，块内删除与插入按出现顺序一一配对，
/// 多出的一侧落空为占位半行。
pub fn pair_rows(rows: &[DiffRow]) -> Vec<PairRow> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        if rows[i].kind == RowKind::Equal {
            out.push(PairRow {
                left: Some(half_from(&rows[i])),
                right: Some(half_from(&rows[i])),
            });
            i += 1;
            continue;
        }
        let start = i;
        while i < rows.len() && rows[i].kind != RowKind::Equal {
            i += 1;
        }
        let block = &rows[start..i];
        let dels: Vec<&DiffRow> = block.iter().filter(|r| r.kind == RowKind::Delete).collect();
        let inss: Vec<&DiffRow> = block.iter().filter(|r| r.kind == RowKind::Insert).collect();
        let n = dels.len().max(inss.len());
        for k in 0..n {
            out.push(PairRow {
                left: dels.get(k).map(|r| half_from(r)),
                right: inss.get(k).map(|r| half_from(r)),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn kinds(rows: &[DiffRow]) -> Vec<RowKind> {
        rows.iter().map(|r| r.kind).collect()
    }

    #[test]
    fn identical_texts_all_equal() {
        let (rows, fast) = compute_rows("a\nb\nc", "a\nb\nc");
        assert!(!fast);
        assert_eq!(kinds(&rows), vec![RowKind::Equal; 3]);
    }

    #[test]
    fn both_empty() {
        let (rows, _) = compute_rows("", "");
        assert!(rows.is_empty());
    }

    #[test]
    fn insert_only() {
        let (rows, _) = compute_rows("a\nc", "a\nb\nc");
        assert_eq!(
            kinds(&rows),
            vec![RowKind::Equal, RowKind::Insert, RowKind::Equal]
        );
    }

    #[test]
    fn delete_only() {
        let (rows, _) = compute_rows("a\nb\nc", "a\nc");
        assert_eq!(
            kinds(&rows),
            vec![RowKind::Equal, RowKind::Delete, RowKind::Equal]
        );
    }

    #[test]
    fn modify_line_produces_delete_then_insert() {
        let (rows, _) = compute_rows("x\nold\nz", "x\nnew\nz");
        assert_eq!(
            kinds(&rows),
            vec![
                RowKind::Equal,
                RowKind::Delete,
                RowKind::Insert,
                RowKind::Equal
            ]
        );
        assert_eq!(rows[1].a_no, Some(2));
        assert_eq!(rows[2].b_no, Some(2));
    }

    #[test]
    fn crlf_and_trailing_newline_tolerated() {
        let (r1, _) = compute_rows("a\r\nb\r\n", "a\nb");
        assert_eq!(kinds(&r1), vec![RowKind::Equal, RowKind::Equal]);
    }

    #[test]
    fn fast_mode_large_input_consistent_counts() {
        let big_a: String = (0..6000)
            .map(|i| format!("line-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut big_b_lines: Vec<String> = (0..6000).map(|i| format!("line-{i}")).collect();
        big_b_lines[3000] = "CHANGED".into();
        let big_b = big_b_lines.join("\n");
        let (rows, fast) = compute_rows(&big_a, &big_b);
        assert!(fast);
        let s = summarize(&rows);
        assert_eq!(s.deleted, 1);
        assert_eq!(s.inserted, 1);
        assert_eq!(s.equal + s.inserted, 6000);
    }

    #[test]
    fn fast_mode_prefix_only_change() {
        let big_a: String = std::iter::once("HEAD".to_string())
            .chain((1..6000).map(|i| format!("same-{i}")))
            .collect::<Vec<_>>()
            .join("\n");
        let big_b: String = std::iter::once("NEW_HEAD".to_string())
            .chain((1..6000).map(|i| format!("same-{i}")))
            .collect::<Vec<_>>()
            .join("\n");
        let (rows, fast) = compute_rows(&big_a, &big_b);
        assert!(fast);
        assert_eq!(kinds(&rows)[..2], [RowKind::Delete, RowKind::Insert]);
        // 尾部等号段两侧行号一致
        let last = rows.last().unwrap();
        assert_eq!(last.a_no, last.b_no);
        assert_eq!(last.kind, RowKind::Equal);
    }

    #[test]
    fn pair_modify_aligns_delete_with_insert() {
        let (rows, _) = compute_rows("x\nold\nz", "x\nnew\nz");
        let p = pair_rows(&rows);
        assert_eq!(p.len(), 3);
        // 中间一行：左删右增
        let mid_left = p[1].left.as_ref().unwrap();
        assert_eq!(mid_left.kind, RowKind::Delete);
        assert_eq!(mid_left.no, 2);
        assert_eq!(mid_left.text, "old");
        let mid_right = p[1].right.as_ref().unwrap();
        assert_eq!(mid_right.kind, RowKind::Insert);
        assert_eq!(mid_right.text, "new");
        // 首尾等号行两侧一致
        assert_eq!(p[0].left, p[0].right);
        assert_eq!(p[2].left, p[2].right);
    }

    #[test]
    fn pair_two_deletes_one_insert_placeholders() {
        // ops 序列固定为 [Del a, Del b, Ins x, Eq c]
        let (rows, _) = compute_rows("a\nb\nc", "x\nc");
        let p = pair_rows(&rows);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].left.as_ref().unwrap().text, "a");
        assert_eq!(p[0].right.as_ref().unwrap().text, "x");
        assert_eq!(p[1].left.as_ref().unwrap().text, "b");
        assert!(p[1].right.is_none());
        assert_eq!(p[2].left.as_ref().unwrap().text, "c");
        assert_eq!(p[2].left, p[2].right);
    }

    #[test]
    fn pair_pure_insert_block_left_placeholder() {
        let (rows, _) = compute_rows("a\nb", "a\nm1\nm2\nb");
        let p = pair_rows(&rows);
        // [Eq a | Ins m1 | Ins m2 | Eq b] -> (a,a)(None,m1)(None,m2)(b,b)
        assert_eq!(p.len(), 4);
        assert!(p[1].left.is_none());
        assert_eq!(p[1].right.as_ref().unwrap().text, "m1");
        assert!(p[2].left.is_none());
        assert_eq!(p[2].right.as_ref().unwrap().text, "m2");
        assert_eq!(p[3].left, p[3].right);
    }
}
