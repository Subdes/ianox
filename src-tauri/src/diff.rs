//! Diff por líneas de lo que una herramienta cambió en un archivo.
//!
//! El backend es el único que ve el contenido anterior y el posterior, así que
//! calcula aquí el diff y la interfaz solo lo pinta (verde/rojo, con números de
//! línea).

use serde::Serialize;

/// Más líneas que esto por lado y el diff completo no compensa: se recorta.
const MAX_SIDE_LINES: usize = 1500;
/// Filas que se mandan a la interfaz; el resto se resume.
const MAX_ROWS: usize = 400;
/// Líneas de contexto alrededor de cada cambio.
const CONTEXT: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiffLine {
    /// `add`, `del` o `ctx`.
    pub kind: &'static str,
    pub text: String,
    /// Número de línea en el archivo anterior (las añadidas no tienen).
    pub old: Option<usize>,
    /// Número de línea en el archivo nuevo (las borradas no tienen).
    pub new: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileDiff {
    pub path: String,
    pub added: usize,
    pub removed: usize,
    /// El archivo era demasiado grande y el diff se recortó.
    pub truncated: bool,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Same,
    Del,
    Add,
}

/// Diff de dos textos. `None` cuando no cambió nada.
pub fn diff(path: &str, before: &str, after: &str) -> Option<FileDiff> {
    if before == after {
        return None;
    }

    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let truncated = old.len() > MAX_SIDE_LINES || new.len() > MAX_SIDE_LINES;

    let ops = if truncated {
        // Demasiado grande para la tabla: todo borrado y todo añadido.
        let mut ops = vec![Op::Del; old.len()];
        ops.extend(vec![Op::Add; new.len()]);
        ops
    } else {
        lcs_ops(&old, &new)
    };

    let added = ops.iter().filter(|op| **op == Op::Add).count();
    let removed = ops.iter().filter(|op| **op == Op::Del).count();

    let rows = rows(&old, &new, &ops);
    Some(FileDiff {
        path: path.to_string(),
        added,
        removed,
        truncated,
        lines: rows,
    })
}

/// Operaciones que llevan `old` a `new`, con la subsecuencia común más larga.
fn lcs_ops(old: &[&str], new: &[&str]) -> Vec<Op> {
    let (n, m) = (old.len(), new.len());
    // table[i][j] = longitud de la LCS de old[i..] y new[j..].
    let stride = m + 1;
    let mut table = vec![0u32; (n + 1) * stride];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i * stride + j] = if old[i] == new[j] {
                table[(i + 1) * stride + j + 1] + 1
            } else {
                table[(i + 1) * stride + j].max(table[i * stride + j + 1])
            };
        }
    }

    let mut ops = Vec::with_capacity(n + m);
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if old[i] == new[j] {
            ops.push(Op::Same);
            i += 1;
            j += 1;
        } else if table[(i + 1) * stride + j] >= table[i * stride + j + 1] {
            ops.push(Op::Del);
            i += 1;
        } else {
            ops.push(Op::Add);
            j += 1;
        }
    }
    ops.extend(std::iter::repeat(Op::Del).take(n - i));
    ops.extend(std::iter::repeat(Op::Add).take(m - j));
    ops
}

/// Convierte las operaciones en filas con contexto y recorta lo que sobra.
fn rows(old: &[&str], new: &[&str], ops: &[Op]) -> Vec<DiffLine> {
    // Filas completas, antes de recortar.
    let mut all: Vec<(Op, usize, usize, &str)> = Vec::with_capacity(ops.len());
    let (mut i, mut j) = (0, 0);
    for op in ops {
        match op {
            Op::Same => {
                all.push((Op::Same, i, j, old[i]));
                i += 1;
                j += 1;
            }
            Op::Del => {
                all.push((Op::Del, i, j, old[i]));
                i += 1;
            }
            Op::Add => {
                all.push((Op::Add, i, j, new[j]));
                j += 1;
            }
        }
    }

    // Qué filas caen dentro del contexto de algún cambio.
    let mut keep = vec![false; all.len()];
    for (index, (op, _, _, _)) in all.iter().enumerate() {
        if *op == Op::Same {
            continue;
        }
        let from = index.saturating_sub(CONTEXT);
        let to = (index + CONTEXT).min(all.len().saturating_sub(1));
        for flag in keep.iter_mut().take(to + 1).skip(from) {
            *flag = true;
        }
    }

    let mut out = Vec::new();
    let mut previous_kept: Option<usize> = None;
    for (index, (op, old_no, new_no, text)) in all.iter().enumerate() {
        if !keep[index] {
            continue;
        }
        if let Some(previous) = previous_kept {
            if index > previous + 1 {
                out.push(DiffLine {
                    kind: "ctx",
                    text: "…".to_string(),
                    old: None,
                    new: None,
                });
            }
        }
        out.push(DiffLine {
            kind: match op {
                Op::Same => "ctx",
                Op::Del => "del",
                Op::Add => "add",
            },
            text: text.to_string(),
            old: (*op != Op::Add).then_some(old_no + 1),
            new: (*op != Op::Del).then_some(new_no + 1),
        });
        previous_kept = Some(index);
    }

    if out.len() > MAX_ROWS {
        out.truncate(MAX_ROWS);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unchanged_file_has_no_diff() {
        assert!(diff("a.txt", "one\ntwo\n", "one\ntwo\n").is_none());
    }

    #[test]
    fn a_new_file_is_all_added() {
        let result = diff("a.txt", "", "one\ntwo\n").unwrap();
        assert_eq!(result.added, 2);
        assert_eq!(result.removed, 0);
        assert!(result.lines.iter().all(|line| line.kind == "add"));
    }

    #[test]
    fn a_changed_line_keeps_its_neighbours_as_context() {
        let before = "keep one\nkeep two\nold line\nkeep three\nkeep four\n";
        let after = "keep one\nkeep two\nnew line\nkeep three\nkeep four\n";
        let result = diff("a.txt", before, after).unwrap();

        assert_eq!(result.added, 1);
        assert_eq!(result.removed, 1);
        let kinds: Vec<&str> = result.lines.iter().map(|line| line.kind).collect();
        assert_eq!(kinds, vec!["ctx", "ctx", "del", "add", "ctx", "ctx"]);
        assert_eq!(result.lines[2].old, Some(3));
        assert_eq!(result.lines[2].new, None);
        assert_eq!(result.lines[3].old, None);
        assert_eq!(result.lines[3].new, Some(3));
    }

    #[test]
    fn a_far_away_change_is_split_with_an_ellipsis() {
        let mut before = String::new();
        for line in 0..40 {
            before.push_str(&format!("line {line}\n"));
        }
        let after = before.replace("line 5\n", "changed\n").replace("line 35\n", "changed\n");
        let result = diff("a.txt", &before, &after).unwrap();
        assert!(result.lines.iter().any(|line| line.text == "…"));
    }

    #[test]
    fn text_is_carried_per_row() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nTWO\nthree\n";
        let result = diff("a.txt", before, after).unwrap();
        let texts: Vec<&str> = result.lines.iter().map(|line| line.text.as_str()).collect();
        assert!(texts.contains(&"two"));
        assert!(texts.contains(&"TWO"));
    }

    #[test]
    fn a_huge_file_is_cut_instead_of_slow() {
        let before: String = (0..3000).map(|i| format!("line {i}\n")).collect();
        let after = before.replace("line 1500\n", "changed\n");
        let result = diff("a.txt", &before, &after).unwrap();
        assert!(result.truncated);
        assert!(result.lines.len() <= MAX_ROWS);
    }
}
