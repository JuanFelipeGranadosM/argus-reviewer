//! Qué problemas introduce un cambio: los diagnósticos actuales que no tenía ya la versión base.
//!
//! Los diagnósticos se emparejan por regla y mensaje (sin números, que se desplazan) y, en las reglas
//! de un diagnóstico por archivo, solo por regla. Un problema que ya existía cuenta como nuevo si
//! empeoró: su métrica (líneas, parámetros, complejidad, usos) supera la de su pareja en la base.

use std::collections::BTreeMap;

use crate::reporters::Diagnostic;

/// Reglas que emiten como mucho un diagnóstico por archivo y cuyo mensaje cambia con el contenido.
const PER_FILE_RULES: &[&str] = &[
    "ARGUS-BLOAT-001",
    "ARGUS-SOUP-001",
    "ARGUS-TYPE-002",
    "ARGUS-TYPE-003",
];

/// Separa `current` en problemas nuevos (o empeorados) y cuenta los que ya existían en `base`.
///
/// Un diagnóstico fuera de las líneas tocadas siempre es previo: lo que no se tocó no pudo empeorar.
/// Los tocados se comparan con lo que queda de la base, de menor a mayor métrica.
pub(super) fn split(
    current: Vec<Diagnostic>,
    base: &[Diagnostic],
    touched: impl Fn(&Diagnostic) -> bool,
) -> (Vec<Diagnostic>, usize) {
    let mut pool: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for diagnostic in base {
        pool.entry(key(diagnostic))
            .or_default()
            .push(diagnostic.metric);
    }
    for metrics in pool.values_mut() {
        metrics.sort_unstable();
    }
    let (mut hit, untouched): (Vec<Diagnostic>, Vec<Diagnostic>) =
        current.into_iter().partition(|d| touched(d));
    for diagnostic in &untouched {
        if !take(&mut pool, diagnostic) {
            if let Some(metrics) = pool.get_mut(&key(diagnostic)) {
                metrics.pop();
            }
        }
    }
    hit.sort_by_key(|d| d.metric);
    let mut preexisting = untouched.len();
    let mut fresh = Vec::new();
    for diagnostic in hit {
        if take(&mut pool, &diagnostic) {
            preexisting += 1;
        } else {
            fresh.push(diagnostic);
        }
    }
    (fresh, preexisting)
}

/// Consume de la base la pareja de `diagnostic` con la menor métrica que no sea inferior a la suya.
fn take(pool: &mut BTreeMap<String, Vec<u32>>, diagnostic: &Diagnostic) -> bool {
    let Some(metrics) = pool.get_mut(&key(diagnostic)) else {
        return false;
    };
    let at = metrics.partition_point(|&m| m < diagnostic.metric);
    if at < metrics.len() {
        metrics.remove(at);
        return true;
    }
    false
}

fn key(diagnostic: &Diagnostic) -> String {
    if PER_FILE_RULES.contains(&diagnostic.id) {
        return diagnostic.id.to_owned();
    }
    let mut message = String::with_capacity(diagnostic.message.len());
    let mut in_number = false;
    for c in diagnostic.message.chars() {
        if c.is_ascii_digit() {
            if !in_number {
                message.push('#');
            }
            in_number = true;
        } else {
            message.push(c);
            in_number = false;
        }
    }
    format!("{}|{message}", diagnostic.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reporters::Severity;

    fn diag(id: &'static str, line: u32, message: &str, metric: u32) -> Diagnostic {
        Diagnostic {
            id,
            severity: Severity::Error,
            rule: "r",
            file: "a.ts".into(),
            line_start: line,
            line_end: line,
            message: message.into(),
            agent_action: None,
            metric,
        }
    }

    fn lines(found: &[Diagnostic]) -> Vec<u32> {
        found.iter().map(|d| d.line_start).collect()
    }

    #[test]
    fn untouched_and_unchanged_problems_are_preexisting() {
        let base = [
            diag("ARGUS-SEC-001", 3, "Captura vacía.", 0),
            diag("ARGUS-SEC-001", 9, "Captura vacía.", 0),
        ];
        let current = vec![
            diag("ARGUS-SEC-001", 4, "Captura vacía.", 0),
            diag("ARGUS-SEC-001", 7, "Captura vacía.", 0),
            diag("ARGUS-SEC-001", 10, "Captura vacía.", 0),
        ];
        let (fresh, old) = split(current, &base, |d| d.line_start == 7);
        assert_eq!((lines(&fresh), old), (vec![7], 2));
    }

    #[test]
    fn a_worse_metric_is_a_new_problem_and_shifted_lines_are_not() {
        let base = [diag(
            "ARGUS-BLOAT-002",
            10,
            "`f` tiene 80 líneas en la línea 10",
            80,
        )];
        let same = vec![diag(
            "ARGUS-BLOAT-002",
            14,
            "`f` tiene 80 líneas en la línea 14",
            80,
        )];
        assert_eq!(split(same, &base, |_| true).1, 1);
        let worse = vec![diag(
            "ARGUS-BLOAT-002",
            14,
            "`f` tiene 95 líneas en la línea 14",
            95,
        )];
        assert_eq!(lines(&split(worse, &base, |_| true).0), [14]);
    }

    #[test]
    fn per_file_rules_pair_by_rule_only() {
        let base = [diag("ARGUS-TYPE-003", 1, "1 función pública: `a`.", 1)];
        let more = vec![diag(
            "ARGUS-TYPE-003",
            1,
            "2 funciones públicas: `a`, `b`.",
            2,
        )];
        assert_eq!(split(more, &base, |_| true).0.len(), 1);
        let fewer = vec![diag("ARGUS-TYPE-003", 5, "1 función pública: `b`.", 1)];
        assert_eq!(split(fewer, &base, |_| true), (vec![], 1));
    }

    #[test]
    fn everything_is_new_without_a_base() {
        let current = vec![diag("ARGUS-SEC-002", 1, "`eval(...)`", 0)];
        assert_eq!(split(current, &[], |_| false).0.len(), 0);
        let current = vec![diag("ARGUS-SEC-002", 1, "`eval(...)`", 0)];
        assert_eq!(split(current, &[], |_| true).0.len(), 1);
    }
}
