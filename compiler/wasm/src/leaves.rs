//! Leaves of the symbol map (SYM-010 to SYM-013, SYM-020).

use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_wasm_ir::{Leaf, ObjId};
use ironplc_wasm_symbols::flags;

use crate::lower::{Lowerer, Member, Section};
use crate::types::Ty;

/// Publishes a member and what it contains under `path`.
pub(crate) fn leaves(
    l: &mut Lowerer,
    path: &str,
    m: &Member,
    object: ObjId,
    base_flags: u32,
) -> Result<(), Diagnostic> {
    if m.section == Section::InOut || m.section == Section::Hidden {
        return Ok(());
    }
    let declared = l.site(&m.span);
    let mut f = base_flags;
    if m.location.is_some() {
        f |= flags::LOCATED;
    }
    let location = m.location.as_ref().map(|(a, _)| a.clone());
    let at = At {
        object,
        declared,
        location,
    };
    tree(l, path, &m.ty, m.offset, f, &at);
    Ok(())
}

struct At {
    object: ObjId,
    declared: u32,
    location: Option<String>,
}

fn tree(l: &mut Lowerer, path: &str, ty: &Ty, offset: u32, f: u32, at: &At) {
    match ty {
        Ty::Scalar { .. } | Ty::Str { .. } | Ty::Ref(_) => push(l, path, ty, offset, f, at, None),
        Ty::Array(a) => {
            if matches!(a.elem, Ty::Scalar { .. } | Ty::Str { .. }) {
                let array = Some((a.dims.clone(), a.stride));
                push(l, path, ty, offset, f, at, array);
            } else {
                for (i, index) in indexes(&a.dims).into_iter().enumerate() {
                    let p = format!("{path}[{}]", index.join(","));
                    tree(l, &p, &a.elem, offset + i as u32 * a.stride, f, at);
                }
            }
        }
        Ty::Struct(s) => {
            for field in &s.fields {
                let p = format!("{path}.{}", field.name);
                tree(l, &p, &field.ty, offset + field.offset, f, at);
            }
        }
        Ty::Fb(name) => {
            let Some(fb) = l.fbs.get(name).cloned() else {
                return;
            };
            for m in &fb.members {
                if matches!(m.section, Section::Hidden | Section::InOut) {
                    continue;
                }
                let p = format!("{path}.{}", m.name);
                let mf = f & !(flags::INPUT | flags::OUTPUT)
                    | match m.section {
                        Section::Input => flags::INPUT,
                        Section::Output => flags::OUTPUT,
                        _ => 0,
                    };
                tree(l, &p, &m.ty, offset + m.offset, mf, at);
            }
        }
    }
}

fn push(
    l: &mut Lowerer,
    path: &str,
    ty: &Ty,
    offset: u32,
    flags: u32,
    at: &At,
    array: Option<(Vec<(i64, i64)>, u32)>,
) {
    let (size, _) = l.size_align(ty);
    let type_name = match (ty, &array) {
        (Ty::Array(a), Some(_)) => a.elem.name(),
        _ => ty.name(),
    };
    let enumeration = l.enums.values(&type_name).cloned();
    l.m.leaves.push(Leaf {
        path: path.to_uppercase(),
        type_name,
        object: at.object,
        offset,
        size,
        flags,
        location: at.location.clone(),
        declared: at.declared,
        enumeration,
        array,
    });
}

/// The index values of every element, in row-major order.
fn indexes(dims: &[(i64, i64)]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = vec![vec![]];
    for (lo, hi) in dims {
        let mut next = vec![];
        for prefix in &out {
            for i in *lo..=*hi {
                let mut p = prefix.clone();
                p.push(i.to_string());
                next.push(p);
            }
        }
        out = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::indexes;

    #[test]
    fn indexes_when_two_dimensions_then_last_varies_fastest() {
        assert_eq!(
            indexes(&[(1, 2), (0, 1)]),
            vec![
                vec!["1", "0"],
                vec!["1", "1"],
                vec!["2", "0"],
                vec!["2", "1"]
            ]
        );
    }
}
