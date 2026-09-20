//! Port of `AbstractUnitConversionRule` (languagetool-core) and the
//! `UnitConversionRule`/`UnitConversionRuleUS` subclasses
//! (`METRIC_UNITS_EN_US`, `tags="picky"`, added by
//! `AmericanEnglish.getRelevantRules` — D-021).
//!
//! The rule works on the raw sentence text. Unit conversions go through a
//! faithful reimplementation of Indriya's converter algebra (`tech.units
//! .indriya` 1.3), because the exact chain composition (rational vs. multiply
//! steps, `Simplifier` fusion, the hardcoded `KILOMETRE_PER_HOUR = 0.277778`)
//! determines the Java suggestion numbers. Number parsing/formatting follows
//! `NumberFormat.getNumberInstance(Locale.US)` with
//! `maximumFractionDigits = 2` and `RoundingMode.HALF_UP`.

use std::sync::OnceLock;

use lt_core::{Match, Suggestion, TextRange};
use regex::Regex;

const RULE_ID: &str = "METRIC_UNITS_EN_US";
const DESCRIPTION: &str = "Suggests or checks conversion of units to their metric equivalents.";
const MESSAGE_SUGGESTION: &str =
    "Writing for an international audience? Consider adding the metric equivalent.";
const SHORT_MESSAGE_SUGGESTION: &str = "Add metric equivalent?";
const MESSAGE_CHECK: &str =
    "This unit conversion doesn't seem right. Do you want to correct it automatically?";
const SHORT_MESSAGE_CHECK: &str = "Incorrect unit conversion. Correct it?";
const MESSAGE_CHECK_UNKNOWN: &str =
    "This unit conversion doesn't seem right, unable to recognize the used unit.";
const SHORT_MESSAGE_CHECK_UNKNOWN: &str = "Unknown unit used in conversion.";
const MESSAGE_UNIT_MISMATCH: &str = "These units don't seem to be compatible.";
const SHORT_MESSAGE_UNIT_MISMATCH: &str = "Units incompatible.";

/// `AbstractUnitConversionRule.DELTA`
const DELTA: f64 = 1e-2;
/// `AbstractUnitConversionRule.ROUNDING_DELTA`
const ROUNDING_DELTA: f64 = 0.05;
/// `AbstractUnitConversionRule.MAX_SUGGESTIONS`
const MAX_SUGGESTIONS: usize = 5;
/// `AbstractUnitConversionRule.WHITESPACE_LIMIT`
const WHITESPACE_LIMIT: usize = 5;

const NUMBER_REGEX: &str = r"(-?[0-9]{1,32}[0-9,.]{0,32})";
const NUMBER_REGEX_WITH_BOUNDARY: &str = r"(-?\b[0-9]{1,32}[0-9,.]{0,32})";

// ---------------------------------------------------------------------------
// Indriya unit algebra (tech.units.indriya 1.3)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dim {
    Mass,
    Length,
    Area,
    Volume,
    Speed,
    Temperature,
    Time,
}

/// One converter step; the chain reads outer-to-inner like Java's
/// `getConversionSteps()` (the last step is applied first).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Identity,
    Rational(i64, i64),
    Multiply(f64),
    Add(f64),
}

impl Step {
    fn is_identity(self) -> bool {
        matches!(self, Step::Identity)
    }

    /// `UnitConverter.isLinear()`: `true` for rational/multiply, and for
    /// `AddConverter` only when it is the identity.
    fn is_linear(self) -> bool {
        match self {
            Step::Identity | Step::Rational(..) | Step::Multiply(_) => true,
            Step::Add(offset) => offset == 0.0,
        }
    }

    /// `Simplifier.normalFormOrder`: Rational (2) before Multiply (4) before
    /// Add (5); identity is 0.
    fn rank(self) -> u8 {
        match self {
            Step::Identity => 0,
            Step::Rational(..) => 2,
            Step::Multiply(_) => 4,
            Step::Add(_) => 5,
        }
    }
}

/// `AbstractUnit.converterOf(double)`: integral factors become rational
/// converters, everything else a multiplier.
fn converter_of(factor: f64) -> Step {
    if is_long_value(factor) {
        Step::Rational(factor as i64, 1)
    } else {
        Step::Multiply(factor)
    }
}

/// `AbstractUnit.isLongValue(double)`.
fn is_long_value(f: f64) -> bool {
    (-9.223_372_036_854_776e18..=9.223_372_036_854_776e18).contains(&f) && f.floor() == f
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.max(1)
}

/// `AbstractConverter.isSimpleCompositionWith` + `simpleCompose` for the
/// step types the rule uses.
fn simple_compose(left: Step, right: Step) -> Option<Step> {
    match (left, right) {
        (Step::Rational(n1, d1), Step::Rational(n2, d2)) => {
            let mut n = n1.checked_mul(n2)?;
            let mut d = d1.checked_mul(d2)?;
            let g = gcd(n, d);
            n /= g;
            d /= g;
            if n == 1 && d == 1 {
                Some(Step::Identity)
            } else {
                Some(Step::Rational(n, d))
            }
        }
        (Step::Multiply(a), Step::Multiply(b)) => Some(Step::Multiply(a * b)),
        (Step::Add(a), Step::Add(b)) => Some(Step::Add(a + b)),
        _ => None,
    }
}

/// One pass of `SimplificationWorker.lambda$trySimplify$0` (identity
/// absorption first, then `simpleCompose`).
fn pair_simplify(left: Step, right: Step) -> Option<Step> {
    if right.is_identity() {
        return Some(left);
    }
    if left.is_identity() {
        return Some(right);
    }
    simple_compose(left, right)
}

/// `Simplifier.sortToNormalFormOrder`: stable-sort each maximal run of
/// linear (rational/multiply) steps by converter class rank; the
/// non-linear `Add` steps separate the runs.
fn sort_to_normal_form_order(list: &mut [Option<Step>]) {
    let mut i = 0;
    while i < list.len() {
        let linear = matches!(list[i], Some(s) if s.is_linear() && !s.is_identity());
        if linear {
            let start = i;
            while i < list.len() && matches!(list[i], Some(s) if s.is_linear() && !s.is_identity())
            {
                i += 1;
            }
            list[start..i].sort_by_key(|s| s.unwrap().rank());
        } else {
            i += 1;
        }
    }
}

/// `AbstractConverter.concatenate` (via `Simplifier.compose`): `a` applied
/// after `b`.
fn compose(a: &[Step], b: &[Step]) -> Vec<Step> {
    let mut list: Vec<Option<Step>> = a.iter().chain(b.iter()).map(|s| Some(*s)).collect();
    loop {
        sort_to_normal_form_order(&mut list);
        let mut changed = 0usize;
        let mut i = 1;
        while i < list.len() {
            if let (Some(left), Some(right)) = (list[i - 1], list[i]) {
                if let Some(result) = pair_simplify(left, right) {
                    list[i - 1] = Some(result);
                    list[i] = None;
                    changed += 1;
                }
            }
            i += 1;
        }
        list.retain(|s| s.is_some());
        if changed == 0 {
            break;
        }
    }
    list.into_iter().map(|s| s.unwrap()).collect()
}

/// `AbstractConverter.inverse()`: reverse the chain and invert each step.
fn invert_steps(steps: &[Step]) -> Vec<Step> {
    steps
        .iter()
        .rev()
        .map(|s| match *s {
            Step::Identity => Step::Identity,
            Step::Rational(n, d) => Step::Rational(d, n),
            Step::Multiply(f) => Step::Multiply(1.0 / f),
            Step::Add(o) => Step::Add(-o),
        })
        .collect()
}

fn convert_steps(steps: &[Step], mut value: f64) -> f64 {
    for step in steps.iter().rev() {
        value = match *step {
            Step::Identity => value,
            Step::Rational(n, d) => value * (n as f64) / (d as f64),
            Step::Multiply(f) => value * f,
            Step::Add(o) => value + o,
        };
    }
    value
}

#[derive(Clone, Debug, PartialEq)]
struct Unit {
    dim: Dim,
    steps: Vec<Step>,
}

impl Unit {
    fn system(dim: Dim) -> Self {
        Self {
            dim,
            steps: Vec::new(),
        }
    }

    /// `AbstractUnit.transform`: build a new unit from this one and a
    /// converter (parent converter chain + converter, simplified).
    fn transform(self, conv: Step) -> Self {
        let steps = compose(&self.steps, &[conv]);
        if steps.iter().all(|s| s.is_identity()) {
            Self::system(self.dim)
        } else {
            Self {
                dim: self.dim,
                steps,
            }
        }
    }

    /// `AbstractUnit.multiply(double)`.
    fn multiply(self, factor: f64) -> Self {
        if factor == 1.0 {
            self
        } else {
            self.transform(converter_of(factor))
        }
    }

    /// `AbstractUnit.multiply(Unit)` (a `ProductUnit`): the composed
    /// converter chain is the concatenation of both chains; the dimension is
    /// the product of the two (only length powers occur in the rule's table).
    fn times(self, other: &Self) -> Self {
        let dim = match (self.dim, other.dim) {
            (Dim::Length, Dim::Length) => Dim::Area,
            (Dim::Length, Dim::Area) | (Dim::Area, Dim::Length) => Dim::Volume,
            (dim, _) => dim,
        };
        Self {
            dim,
            steps: compose(&self.steps, &other.steps),
        }
    }

    /// `AbstractUnit.divide(double)`.
    fn divide(self, divisor: f64) -> Self {
        if divisor == 1.0 {
            self
        } else if is_long_value(divisor) {
            self.transform(Step::Rational(1, divisor as i64))
        } else {
            self.transform(Step::Multiply(1.0 / divisor))
        }
    }

    /// `AbstractUnit.shift(double)`.
    fn shift(self, offset: f64) -> Self {
        if offset == 0.0 {
            self
        } else {
            self.transform(Step::Add(offset))
        }
    }

    /// Quotient of two units (`a.divide(b)`); the system dimension is given
    /// explicitly because only speed (`m/s`, `mile/h`) uses it.
    fn quotient(a: &Self, b: &Self, dim: Dim) -> Self {
        let steps = compose(&a.steps, &invert_steps(&b.steps));
        Self { dim, steps }
    }

    fn is_compatible(&self, other: &Self) -> bool {
        self.dim == other.dim
    }

    /// `unit.getConverterTo(other).convert(value)`; `None` stands for
    /// Indriya's `UnconvertibleException` (incompatible dimensions).
    fn convert_to(&self, other: &Self, value: f64) -> Option<f64> {
        if !self.is_compatible(other) {
            return None;
        }
        let composite = compose(&invert_steps(&other.steps), &self.steps);
        Some(convert_steps(&composite, value))
    }
}

// ---------------------------------------------------------------------------
// Rule configuration (rule id/messages/number locale/unit table language)
// ---------------------------------------------------------------------------

/// `UnitConversionRule` variants: the English-US metric rule (`METRIC_UNITS_
/// EN_US`, added by `AmericanEnglish`) and the German one (`EINHEITEN_
/// METRISCH`, `org.languagetool.rules.de.UnitConversionRule`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UnitLang {
    EnUs,
    De,
    Pt,
}

/// `NumberFormat.getNumberInstance(locale)`: US (`1,000.5`) vs Germany
/// (`1.000,5`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NumberLocale {
    EnUs,
    Germany,
}

pub struct UnitConversionConfig {
    pub lang: UnitLang,
    pub locale: NumberLocale,
    pub rule_id: &'static str,
    pub description: &'static str,
    pub message_suggestion: &'static str,
    pub short_message_suggestion: &'static str,
    pub message_check: &'static str,
    pub short_message_check: &'static str,
    pub message_check_unknown: &'static str,
    pub short_message_check_unknown: &'static str,
    pub message_unit_mismatch: &'static str,
    pub short_message_unit_mismatch: &'static str,
    /// `formatRounded` prefix ("ca. " base, "aprox. " for Portuguese)
    pub rounded_prefix: &'static str,
}

pub static CONFIG_EN_US: UnitConversionConfig = UnitConversionConfig {
    lang: UnitLang::EnUs,
    locale: NumberLocale::EnUs,
    rule_id: RULE_ID,
    description: DESCRIPTION,
    message_suggestion: MESSAGE_SUGGESTION,
    short_message_suggestion: SHORT_MESSAGE_SUGGESTION,
    message_check: MESSAGE_CHECK,
    short_message_check: SHORT_MESSAGE_CHECK,
    message_check_unknown: MESSAGE_CHECK_UNKNOWN,
    short_message_check_unknown: SHORT_MESSAGE_CHECK_UNKNOWN,
    message_unit_mismatch: MESSAGE_UNIT_MISMATCH,
    short_message_unit_mismatch: SHORT_MESSAGE_UNIT_MISMATCH,
    rounded_prefix: "ca. ",
};

/// `org.languagetool.rules.de.UnitConversionRule` (`EINHEITEN_METRISCH`).
pub static CONFIG_DE: UnitConversionConfig = UnitConversionConfig {
    lang: UnitLang::De,
    locale: NumberLocale::Germany,
    rule_id: "EINHEITEN_METRISCH",
    description: "Schlägt vor oder überprüft Angaben des metrischen Äquivalentes bei bestimmten Maßeinheiten.",
    message_suggestion: "Wollen Sie eine Umwandlung ins metrische System automatisch hinzufügen?",
    short_message_suggestion: "Metrisches Äquivalent hinzufügen?",
    message_check: "Diese Umrechnung scheint falsch zu sein. Wollen Sie sie automatisch korrigieren lassen?",
    short_message_check: "Falsche Umrechnung. Automatisch korrigieren?",
    message_check_unknown: "Die in dieser Umrechnung verwendete Einheit wurde nicht erkannt.",
    short_message_check_unknown: "Unbekannte Einheit.",
    message_unit_mismatch: "Diese Einheiten sind nicht kompatibel.",
    short_message_unit_mismatch: "Inkompatible Einheiten.",
    rounded_prefix: "ca. ",
};

/// `org.languagetool.rules.pt.PortugueseUnitConversionRule` (`UNIDADES_METRICAS`).
pub static CONFIG_PT: UnitConversionConfig = UnitConversionConfig {
    lang: UnitLang::Pt,
    // `NumberFormat.getNumberInstance(Locale.GERMANY)`
    locale: NumberLocale::Germany,
    rule_id: "UNIDADES_METRICAS",
    description:
        "Sugere ou verifica informações equivalentes à métrica de unidades de medida específicas.",
    message_suggestion: "Deseja adicionar automaticamente uma conversão ao sistema métrico?",
    short_message_suggestion: "Adicionar conversão ao sistema métrico?",
    message_check: "Esta conversão não parece estar precisa. Gostaria de corrigi-la?",
    short_message_check: "Conversão incorreta. Corrigir?",
    message_check_unknown: "A unidade usada nesta conversão não foi reconhecida.",
    short_message_check_unknown: "Unidade desconhecida.",
    message_unit_mismatch: "Estas unidades de medida não são compatíveis.",
    short_message_unit_mismatch: "Unidade incompatível.",
    rounded_prefix: "aprox. ",
};

// ---------------------------------------------------------------------------
// Number parsing/formatting (NumberFormat, locale-specific)
// ---------------------------------------------------------------------------

/// `NumberFormat.getNumberInstance(locale).parse(String)`: grouping
/// separators are ignored, parsing stops at the first invalid character, and
/// the parsed prefix is returned (`"1.2.3"` parses as `1.2` in en-US, `123`
/// in de-DE).
fn parse_number(locale: NumberLocale, s: &str) -> Option<f64> {
    let (grouping, decimal): (u8, u8) = match locale {
        NumberLocale::EnUs => (b',', b'.'),
        NumberLocale::Germany => (b'.', b','),
    };
    let bytes = s.as_bytes();
    let mut i = 0;
    let negative = bytes.first() == Some(&b'-');
    if negative {
        i += 1;
    }
    let mut int_digits = String::new();
    let mut frac_digits: Option<String> = None;
    while i < bytes.len() {
        match bytes[i] {
            b'0'..=b'9' => {
                if let Some(frac) = frac_digits.as_mut() {
                    frac.push(bytes[i] as char);
                } else {
                    int_digits.push(bytes[i] as char);
                }
            }
            b if b == grouping && frac_digits.is_none() => {}
            b if b == decimal && frac_digits.is_none() => frac_digits = Some(String::new()),
            _ => break,
        }
        i += 1;
    }
    if int_digits.is_empty() && frac_digits.as_ref().is_none_or(|f| f.is_empty()) {
        return None;
    }
    let mut text = int_digits;
    text.push('.');
    if let Some(frac) = frac_digits {
        text.push_str(&frac);
    } else {
        text.push('0');
    }
    let value: f64 = text.parse().ok()?;
    Some(if negative { -value } else { value })
}

fn increment_digits(digits: &mut Vec<u8>) {
    for digit in digits.iter_mut().rev() {
        if *digit < b'9' {
            *digit += 1;
            return;
        }
        *digit = b'0';
    }
    digits.insert(0, b'1');
}

fn group_digits(digits: &str, separator: char) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    let first = match digits.len() % 3 {
        0 => 3,
        rem => rem,
    };
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (i as isize - first as isize) % 3 == 0 {
            out.push(separator);
        }
        out.push(c);
    }
    out
}

/// `NumberFormat.getNumberInstance(Locale.GERMANY)` with
/// `maximumFractionDigits = 2`, `RoundingMode.HALF_UP`, grouping on.
fn format_decimal_locale(locale: NumberLocale, value: f64) -> String {
    const MAX_FRACTION_DIGITS: usize = 2;
    let negative = value.is_sign_negative();
    let abs = value.abs();
    if !abs.is_finite() {
        // Java formats NaN/Infinity; the rule never produces them (guarded by
        // the comparison that produces the number), keep a safe fallback.
        return if abs.is_nan() {
            "NaN".to_string()
        } else if negative {
            "-∞".to_string()
        } else {
            "∞".to_string()
        };
    }
    // 22 extra digits are enough to decide HALF_UP on the exact binary value
    // (doubles have at most ~17 significant decimal digits).
    let rendered = format!("{:.*}", MAX_FRACTION_DIGITS + 22, abs);
    let (int_part, frac_part) = rendered.split_once('.').unwrap_or((&rendered, ""));
    let mut digits: Vec<u8> = int_part.bytes().collect();
    digits.extend(frac_part[..MAX_FRACTION_DIGITS].bytes());
    if frac_part
        .as_bytes()
        .get(MAX_FRACTION_DIGITS)
        .is_some_and(|d| *d >= b'5')
    {
        increment_digits(&mut digits);
    }
    let int_len = digits.len() - MAX_FRACTION_DIGITS;
    let int_digits = String::from_utf8(digits[..int_len].to_vec()).unwrap();
    let frac_digits = String::from_utf8(digits[int_len..].to_vec()).unwrap();
    let frac_trimmed = frac_digits.trim_end_matches('0');
    let (grouping, decimal) = match locale {
        NumberLocale::EnUs => (',', '.'),
        NumberLocale::Germany => ('.', ','),
    };
    let mut out = group_digits(&int_digits, grouping);
    if !frac_trimmed.is_empty() {
        out.push(decimal);
        out.push_str(frac_trimmed);
    }
    if negative {
        out.insert(0, '-');
    }
    out
}

/// `NumberFormat.format(long)` (grouping, no fraction digits).
fn format_long_locale(locale: NumberLocale, value: i64) -> String {
    let negative = value < 0;
    let digits = value.unsigned_abs().to_string();
    let separator = match locale {
        NumberLocale::EnUs => ',',
        NumberLocale::Germany => '.',
    };
    let mut out = group_digits(&digits, separator);
    if negative {
        out.insert(0, '-');
    }
    out
}

/// `Math.round(double)`: `floor(x + 0.5)`, saturating at the long bounds.
fn java_round(value: f64) -> i64 {
    if value.is_nan() {
        return 0;
    }
    if value >= 9.223_372_036_854_776e18 {
        return i64::MAX;
    }
    if value <= -9.223_372_036_854_776e18 {
        return i64::MIN;
    }
    (value + 0.5).floor() as i64
}

/// Java `String.trim()` (removes chars `<= U+0020`).
fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| (c as u32) <= 0x20)
}

// ---------------------------------------------------------------------------
// Unit table
// ---------------------------------------------------------------------------

struct UnitPattern {
    re: Regex,
    unit: Unit,
    /// Java appends `\b` after the pattern and `ft`/`inch` carry a
    /// `(?!(\w|\d))` lookahead; for the `'`/`″` alternatives the lookahead
    /// makes the trailing `\b` unsatisfiable, so the next character is
    /// re-checked here (see D-021).
    trailing_not_word: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpecialKind {
    /// `(?:(?<=[^º°\d]))\s(\d+)…` — match starts at the whitespace.
    LeadingSpace,
    /// `(?:(?<=[^º°\d\s]))(\d+)…` — match starts at the first digit.
    NoLeadingSpace,
}

struct UnitTable {
    patterns: Vec<UnitPattern>,
    symbols: Vec<(Unit, Vec<&'static str>)>,
    metric_units: Vec<Unit>,
    special: [(Regex, SpecialKind); 2],
    number_range_part: Regex,
    anti_patterns: Vec<Regex>,
    converted: Regex,
    feet_inch_conversion: Regex,
    /// `AbstractUnitConversionRule.FEET`, used by the special `5'6"` patterns.
    feet: Unit,
}

#[derive(Default)]
struct Builder {
    patterns: Vec<UnitPattern>,
    symbols: Vec<(Unit, Vec<&'static str>)>,
    metric_units: Vec<Unit>,
}

impl Builder {
    /// `AbstractUnitConversionRule.addUnit`.
    fn add(&mut self, pattern: &str, base: &Unit, symbol: &'static str, factor: f64, metric: bool) {
        self.add_unit(pattern, base.clone().multiply(factor), symbol, metric);
    }

    fn add_unit(&mut self, pattern: &str, unit: Unit, symbol: &'static str, metric: bool) {
        let trailing_not_word = pattern.contains("(?!(\\w|\\d))");
        let pattern = pattern.replace("(?!(\\w|\\d))", "");
        let re = Regex::new(&format!(
            r"{NUMBER_REGEX_WITH_BOUNDARY}[\s\u{{00A0}}]{{0,{WHITESPACE_LIMIT}}}{pattern}\b"
        ))
        .unwrap_or_else(|e| panic!("unit pattern {pattern}: {e}"));
        match self.symbols.iter_mut().find(|(u, _)| *u == unit) {
            Some((_, symbols)) => symbols.push(symbol),
            None => self.symbols.push((unit.clone(), vec![symbol])),
        }
        if metric && !self.metric_units.contains(&unit) {
            self.metric_units.push(unit.clone());
        }
        self.patterns.push(UnitPattern {
            re,
            unit,
            trailing_not_word,
        });
    }
}

fn build_table(lang: UnitLang) -> UnitTable {
    let mut b = Builder::default();

    let kilogram = Unit::system(Dim::Mass);
    let metre = Unit::system(Dim::Length);
    let square_metre = Unit::system(Dim::Area);
    let cubic_metre = Unit::system(Dim::Volume);
    let second = Unit::system(Dim::Time);
    let kelvin = Unit::system(Dim::Temperature);
    let metre_per_second = Unit::system(Dim::Speed);
    let litre = cubic_metre.clone().divide(1000.0);
    let hour = second.clone().multiply(3600.0);

    // `AbstractUnitConversionRule` constructor.
    let pound = kilogram.clone().multiply(0.45359237);
    let ounce = pound.clone().divide(12.0);
    let feet = metre.clone().multiply(0.3048);
    let yard = feet.clone().multiply(3.0);
    let inch = feet.clone().divide(12.0);
    let mile = feet.clone().multiply(5280.0);
    let us_quart = litre.clone().multiply(0.946352946);
    let us_gallon = us_quart.clone().multiply(4.0);
    let us_pint = us_quart.clone().divide(2.0);
    let us_cup = us_quart.clone().divide(4.0);
    let us_fl_ounce = us_quart.clone().divide(32.0);
    let fahrenheit = kelvin
        .clone()
        .shift(273.15)
        .multiply(5.0 / 9.0)
        .shift(-32.0);
    let celsius = kelvin.clone().shift(273.15);
    let kilometre_per_hour = metre_per_second.clone().multiply(0.277778);
    let miles_per_hour = Unit::quotient(&mile, &hour, Dim::Speed);

    b.add("kg", &kilogram, "kg", 1e0, true);
    b.add("g", &kilogram, "g", 1e-3, true);
    b.add("t", &kilogram, "t", 1e3, true);

    b.add("lb", &pound, "lb", 1.0, false);
    b.add("mi", &mile, "mi", 1.0, false);
    b.add("yd", &yard, "yd", 1.0, false);
    b.add("(?:ft|′|')(?!(\\w|\\d))", &feet, "ft", 1.0, false);
    b.add("(?:inch|″)(?!(\\w|\\d))", &inch, "inch", 1.0, false);

    b.add("(?:km/h|kmh)", &kilometre_per_hour, "km/h", 1.0, true);
    b.add("(?:mph)", &miles_per_hour, "mph", 1.0, false);

    b.add("km", &metre, "km", 1e3, true);
    b.add("m", &metre, "m", 1e0, true);
    b.add("cm", &metre, "cm", 1e-2, true);
    b.add("mm", &metre, "mm", 1e-3, true);
    b.add("µm", &metre, "µm", 1e-6, true);
    b.add("nm", &metre, "nm", 1e-9, true);

    b.add("m(?:\\^2|2|²)", &square_metre, "m²", 1.0, true);
    b.add("ha", &square_metre, "ha", 1e4, true);
    b.add("a", &square_metre, "a", 1e2, true);
    b.add("km(?:\\^2|2|²)", &square_metre, "km²", 1e6, true);
    b.add("cm(?:\\^2|2|²)", &square_metre, "cm²", 1e-4, true);
    b.add("mm(?:\\^2|2|²)", &square_metre, "mm²", 1e-6, true);
    b.add("µm(?:\\^2|2|²)", &square_metre, "µm²", 1e-12, true);
    b.add("nm(?:\\^2|2|²)", &square_metre, "nm²", 1e-18, true);

    let square_inch = inch.clone().times(&inch);
    let square_feet = feet.clone().times(&feet);
    let square_yard = yard.clone().times(&yard);
    b.add(
        "(?:sq|square) (?:in(?:ch)?|inches)",
        &inch.clone().times(&inch),
        "sq in",
        1.0,
        false,
    );
    b.add(
        "(?:inches|in|inch) (?:\\^2|2|²)",
        &square_inch,
        "in²",
        1.0,
        false,
    );

    b.add(
        "(?:sq|square) (?:ft|feet|foot)",
        &square_feet,
        "sq ft",
        1.0,
        false,
    );
    b.add("sf", &square_feet, "sf", 1.0, false);
    b.add(
        "ft(?:\\^2|2|²)",
        &feet.clone().times(&feet),
        "ft²",
        1.0,
        false,
    );

    b.add(
        "(?:sq|square) (?:yds?|yards?)",
        &square_yard,
        "sq yd",
        1.0,
        false,
    );
    b.add(
        "(?:yards?|yds?)(?:\\^2|2|²)",
        &yard.clone().times(&yard),
        "yd²",
        1.0,
        false,
    );

    b.add("m(?:\\^3|3|³)", &cubic_metre, "m³", 1.0, true);
    b.add("km(?:\\^3|3|³)", &cubic_metre, "km³", 1e9, true);
    b.add("cm(?:\\^3|3|³)", &cubic_metre, "cm³", 1e-6, true);
    b.add("mm(?:\\^3|3|³)", &cubic_metre, "mm³", 1e-9, true);
    b.add("µm(?:\\^3|3|³)", &cubic_metre, "µm³", 1e-18, true);
    b.add("nm(?:\\^3|3|³)", &cubic_metre, "nm³", 1e-27, true);

    let cubic_feet = feet.clone().times(&feet).times(&feet);
    let cubic_inch = inch.clone().times(&inch).times(&inch);
    let cubic_yard = yard.clone().times(&yard).times(&yard);
    b.add(
        "(?:cubic|cu) (?:feet|ft|foot)",
        &cubic_feet,
        "cubic feet",
        1.0,
        false,
    );
    b.add(
        "(?:feet|ft|foot)(?:\\^3|3|³)",
        &feet.clone().times(&feet).times(&feet),
        "ft³",
        1.0,
        false,
    );

    b.add(
        "(?:cubic|cu) (?:inch|in|inches)",
        &cubic_inch,
        "cubic inch",
        1.0,
        false,
    );
    b.add(
        "(?:inch|in)(?:\\^3|3|³)",
        &inch.clone().times(&inch).times(&inch),
        "inch³",
        1.0,
        false,
    );

    b.add(
        "(?:cubic|cu) (?:yards?|yds?)",
        &cubic_yard,
        "cubic yard",
        1.0,
        false,
    );
    b.add(
        "(?:yard|yd)(?:\\^3|3|³)",
        &yard.clone().times(&yard).times(&yard),
        "yard³",
        1.0,
        false,
    );

    b.add("l", &litre, "l", 1.0, true);
    b.add("ml", &litre, "ml", 1e-3, true);

    b.add("°F", &fahrenheit, "°F", 1.0, false);
    b.add("°C", &celsius, "°C", 1.0, true);

    if lang == UnitLang::De {
        // `org.languagetool.rules.de.UnitConversionRule` constructor.
        b.add("Kilo(gramm)?", &kilogram, "Kilogramm", 1e0, true);
        b.add("Gramm", &kilogram, "Gramm", 1e-3, true);
        b.add("Tonnen?", &kilogram, "Tonnen", 1e3, true);
        b.add("Pfund", &pound, "Pfund", 1.0, false);

        b.add("Meilen?", &mile, "Meile", 1.0, false);
        b.add("Yard", &yard, "Yard", 1.0, false);
        b.add("Fuß", &feet, "Fuß", 1.0, false);
        b.add("Zoll", &inch, "Zoll", 1.0, false);

        b.add(
            "(Kilometer pro Stunde|Stundenkilometer)",
            &kilometre_per_hour,
            "Kilometer pro Stunde",
            1.0,
            true,
        );
        b.add(
            "Meilen pro Stunde",
            &Unit::quotient(&mile, &hour, Dim::Speed),
            "Meilen pro Stunde",
            1.0,
            false,
        );

        b.add("Meter", &metre, "Meter", 1e0, true);
        b.add("Kilometer", &metre, "Kilometer", 1e3, true);
        b.add("Zentimeter", &metre, "Zentimeter", 1e-2, true);
        b.add("Millimeter", &metre, "Millimeter", 1e-3, true);
        b.add("Mikrometer", &metre, "Mikrometer", 1e-6, true);
        b.add("Nanometer", &metre, "Nanometer", 1e-9, true);
        b.add("Pikometer", &metre, "Pikometer", 1e-12, true);
        b.add("Femtometer", &metre, "Femtometer", 1e-15, true);

        b.add("Quadratmeter", &square_metre, "Quadratmeter", 1.0, true);
        b.add("Hektar", &square_metre, "Hektar", 1e4, true);
        b.add("Ar", &square_metre, "Ar", 1e2, true);
        b.add(
            "Quadratkilometer",
            &square_metre,
            "Quadratkilometer",
            1e6,
            true,
        );
        b.add(
            "Quadratzentimeter",
            &square_metre,
            "Quadratzentimeter",
            1e-4,
            true,
        );
        b.add(
            "Quadratmillimeter",
            &square_metre,
            "Quadratmillimeter",
            1e-6,
            true,
        );
        b.add(
            "Quadratmikrometer",
            &square_metre,
            "Quadratmikrometer",
            1e-12,
            true,
        );
        b.add(
            "Quadratnanometer",
            &square_metre,
            "Quadratnanometer",
            1e-18,
            true,
        );

        b.add("Kubikmeter", &cubic_metre, "Kubikmeter", 1.0, true);
        b.add("Kubikkilometer", &cubic_metre, "Kubikkilometer", 1e9, true);
        b.add(
            "Kubikzentimeter",
            &cubic_metre,
            "Kubikzentimeter",
            1e-6,
            true,
        );
        b.add(
            "Kubikmillimeter",
            &cubic_metre,
            "Kubikmillimeter",
            1e-9,
            true,
        );
        b.add(
            "Kubikmikrometer",
            &cubic_metre,
            "Kubikmikrometer",
            1e-18,
            true,
        );
        b.add(
            "Kubiknanometer",
            &cubic_metre,
            "Kubiknanometer",
            1e-27,
            true,
        );

        b.add("Liter", &litre, "Liter", 1.0, true);
        b.add("Milliliter", &litre, "Milliliter", 1e-3, true);

        b.add(
            "(?:Grad)? Fahrenheit",
            &fahrenheit,
            "Grad Fahrenheit",
            1.0,
            false,
        );
        b.add("(?:Grad)? Celsius", &celsius, "Grad Celsius", 1.0, true);
    }

    if lang == UnitLang::Pt {
        // `PortugueseUnitConversionRule` constructor.
        b.add("(qui|ki)lo(grama)?", &kilogram, "quilogramas", 1e0, true);
        b.add("grama", &kilogram, "gramas", 1e-3, true);
        b.add("toneladas?", &kilogram, "toneladas", 1e3, true);
        b.add("libras?", &pound, "libras", 1.0, false);
        b.add("onças?", &ounce, "onças", 1.0, false);

        b.add("milhas?", &mile, "milhas", 1.0, false);
        b.add("jardas?", &yard, "jardas", 1.0, false);
        b.add("pés?", &feet, "pés", 1.0, false);
        b.add("polegadas?", &inch, "polegadas", 1.0, false);

        b.add(
            "(qu|k)ilômetros? por hora",
            &kilometre_per_hour,
            "quilômetros por hora",
            1.0,
            true,
        );
        b.add(
            "milhas? por hora",
            &Unit::quotient(&mile, &hour, Dim::Speed),
            "milhas por hora",
            1.0,
            false,
        );

        b.add("metros?", &metre, "metros", 1e0, true);
        b.add("(qu|k)ilômetros?", &metre, "quilômetros", 1e3, true);
        // metric, but should not be suggested
        b.add("decímetros?", &metre, "decímetros", 1e-1, false);
        b.add("centímetros?", &metre, "centímetros", 1e-2, true);
        b.add("milímetros?", &metre, "milímetros", 1e-3, true);
        b.add("micrômetros?", &metre, "micrômetros", 1e-6, true);
        b.add("nanômetros?", &metre, "nanômetros", 1e-9, true);
        b.add("picômetros?", &metre, "picômetros", 1e-12, true);
        b.add("fentômetros?", &metre, "fentômetros", 1e-15, true);

        b.add(
            "metros? quadrados?",
            &square_metre,
            "metros quadrados",
            1.0,
            true,
        );
        b.add("hectar(es)?", &square_metre, "hectares", 1e4, true);
        b.add("ares?", &square_metre, "ares", 1e2, true);
        b.add(
            "(k|qui)ilômetros? quadrados?",
            &square_metre,
            "quilômetros quadrados",
            1e6,
            true,
        );
        // Metric, but not commonly used
        b.add(
            "decímetros? quadrados?",
            &square_metre,
            "decímetros quadrados",
            1e-2,
            false,
        );
        b.add(
            "centímetros? quadrados?",
            &square_metre,
            "centímetros quadrados",
            1e-4,
            true,
        );
        b.add(
            "milímetros? quadrados?",
            &square_metre,
            "milímetros quadrados",
            1e-6,
            true,
        );
        b.add(
            "micrômetros? quadrados?",
            &square_metre,
            "micrômetros quadrados",
            1e-12,
            true,
        );
        b.add(
            "nanômetros? quadrados?",
            &square_metre,
            "nanômetros quadrados",
            1e-18,
            true,
        );

        b.add(
            "metros? cúbicos?",
            &cubic_metre,
            "metros cúbicos",
            1.0,
            true,
        );
        b.add(
            "(k|qu)ilômetros? cúbicos?",
            &cubic_metre,
            "quilômetros cúbicos",
            1e9,
            true,
        );
        b.add(
            "decímetros? cúbicos?",
            &cubic_metre,
            "decímetros cúbicos",
            1e-3,
            false,
        );
        b.add(
            "centímetros? cúbicos?",
            &cubic_metre,
            "centímetros cúbicos",
            1e-6,
            true,
        );
        b.add(
            "milímetros? cúbicos?",
            &cubic_metre,
            "milímetros cúbicos",
            1e-9,
            true,
        );
        b.add(
            "micrômetros? cúbicos?",
            &cubic_metre,
            "micrômetros cúbicos",
            1e-18,
            true,
        );
        b.add(
            "nanômetros? cúbicos?",
            &cubic_metre,
            "nanômetros cúbicos",
            1e-27,
            true,
        );

        b.add("litros?", &litre, "litros", 1.0, true);
        b.add("mililitros?", &litre, "mililitros", 1e-3, true);

        b.add(
            "(?:Graus)? Fahrenheit",
            &fahrenheit,
            "graus Fahrenheit",
            1.0,
            false,
        );
        b.add(
            "(?:Graus)? (Celsi[ou]s|[cC]entígrados?)",
            &celsius,
            "graus Celsius",
            1.0,
            true,
        );
    }

    if lang == UnitLang::EnUs {
        // `UnitConversionRule` constructor.
        b.add(
            "miles per hour",
            &miles_per_hour,
            "miles per hour",
            1.0,
            false,
        );

        b.add("kilograms?", &kilogram, "kilogram", 1e0, true);
        b.add("grams?", &kilogram, "gram", 1e-3, true);
        b.add("tons?", &kilogram, "ton", 1e3, true);

        b.add("pounds?", &pound, "pounds", 1.0, false);
        b.add("ounces?", &ounce, "ounces", 1.0, false);

        b.add("feet", &feet, "feet", 1.0, false);
        b.add("miles?", &mile, "miles", 1.0, false);
        b.add("yards?", &yard, "yards", 1.0, false);
        b.add("inch(es)?", &inch, "inches", 1.0, false);

        b.add(
            "(?:degrees?)? Fahrenheit",
            &fahrenheit,
            "degree Fahrenheit",
            1.0,
            false,
        );
        b.add(
            "(?:degrees?)? Celsius",
            &celsius,
            "degree Celsius",
            1.0,
            true,
        );

        // `UnitConversionRuleUS` constructor.
        b.add(
            "(kilometre|kilometer)s? per hour",
            &kilometre_per_hour,
            "kilometers per hour",
            1.0,
            true,
        );

        b.add("kilomet(re|er)s?", &metre, "kilometers", 1e3, true);
        b.add("met(re|er)s?", &metre, "meters", 1e0, true);
        b.add("decimet(re|er)s?", &metre, "decimeters", 1e-1, false);
        b.add("centimet(re|er)s?", &metre, "centimeters", 1e-2, true);
        b.add("millimet(re|er)s?", &metre, "micrometers", 1e-3, true);
        b.add("micromet(re|er)s?", &metre, "micrometers", 1e-6, true);
        b.add("nanomet(re|er)s?", &metre, "nanometers", 1e-9, true);

        b.add(
            "square met(re|er)s?",
            &square_metre,
            "square meters",
            1.0,
            true,
        );
        b.add(
            "square kilomet(re|er)s?",
            &square_metre,
            "square kilometers",
            1e6,
            true,
        );
        b.add(
            "square decimet(re|er)s?",
            &square_metre,
            "square decimeters",
            1e-2,
            false,
        );
        b.add(
            "square centimet(re|er)s?",
            &square_metre,
            "square centimeters",
            1e-4,
            true,
        );
        b.add(
            "square millimet(re|er)s?",
            &square_metre,
            "square millimeters",
            1e-6,
            true,
        );
        b.add(
            "square micromet(re|er)s?",
            &square_metre,
            "square micrometers",
            1e-12,
            true,
        );
        b.add(
            "square nanomet(re|er)s?",
            &square_metre,
            "square nanometers",
            1e-18,
            true,
        );

        b.add(
            "cubic met(re|er)s?",
            &cubic_metre,
            "cubic meters",
            1.0,
            true,
        );
        b.add(
            "cubic kilomet(re|er)s?",
            &cubic_metre,
            "cubic kilometers",
            1e9,
            true,
        );
        b.add(
            "cubic decimet(re|er)s?",
            &cubic_metre,
            "cubic decimeters",
            1e-3,
            false,
        );
        b.add(
            "cubic centimet(re|er)s?",
            &cubic_metre,
            "cubic centimeters",
            1e-6,
            true,
        );
        b.add(
            "cubic millimet(re|er)s?",
            &cubic_metre,
            "cubic millimeters",
            1e-9,
            true,
        );
        b.add(
            "cubic micromet(re|er)s?",
            &cubic_metre,
            "cubic micrometers",
            1e-18,
            true,
        );
        b.add(
            "cubic nanomet(re|er)s?",
            &cubic_metre,
            "cubic nanometers",
            1e-27,
            true,
        );

        b.add("lit(re|er)s?", &litre, "liters", 1.0, true);
        b.add("millilit(re|er)s?", &litre, "milliliters", 1e-3, true);

        b.add("qt\\.", &us_quart, "qt.", 1.0, false);
        b.add("gal", &us_gallon, "gal", 1.0, false);
        b.add("pt", &us_pint, "pt", 1.0, false);
        b.add("cup", &us_cup, "cups", 1.0, false);
        b.add("(?:fl.? oz.?|oz. fl.)", &us_fl_ounce, "fl oz", 1.0, false);

        b.add("quarts?", &us_quart, "quarts", 1.0, false);
        b.add("gallons?", &us_gallon, "gallons", 1.0, false);
        b.add("pints?", &us_pint, "pints", 1.0, false);
        b.add("cups?", &us_cup, "cups", 1.0, false);
        b.add("(fluid )?ounces?", &us_fl_ounce, "fluid ounces", 1.0, false);
    }

    UnitTable {
        patterns: b.patterns,
        symbols: b.symbols,
        metric_units: b.metric_units,
        special: [
            (
                Regex::new(r#"\s(\d+)(?:ft|′|')\s*(\d+)\s*(?:in|"|″)?"#).unwrap(),
                SpecialKind::LeadingSpace,
            ),
            (
                Regex::new(r#"(\d+)(?:ft|′|')\s*(\d+)\s*(?:in|"|″)?"#).unwrap(),
                SpecialKind::NoLeadingSpace,
            ),
        ],
        number_range_part: Regex::new(&format!("{NUMBER_REGEX_WITH_BOUNDARY}$")).unwrap(),
        anti_patterns: [
            r"\s?\d+'\d\d\d\s?",
            r"\d+[-‐–]\d+",
            r"\d+/\d+",
            r"\d+:\d+",
            r"Pfund Sterling",
            r"\d+⁄\d+",
        ]
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect(),
        converted: Regex::new(&format!(r"\s*\((?:ca. )?{NUMBER_REGEX}\s*([^)]+)\s*\)")).unwrap(),
        feet_inch_conversion: Regex::new(r"^\(\d+ (feet|ft) \d+ inch\)$").unwrap(),
        feet,
    }
}

fn table_for(lang: UnitLang) -> &'static UnitTable {
    static EN_US: OnceLock<UnitTable> = OnceLock::new();
    static DE: OnceLock<UnitTable> = OnceLock::new();
    static PT: OnceLock<UnitTable> = OnceLock::new();
    match lang {
        UnitLang::EnUs => EN_US.get_or_init(|| build_table(UnitLang::EnUs)),
        UnitLang::De => DE.get_or_init(|| build_table(UnitLang::De)),
        UnitLang::Pt => PT.get_or_init(|| build_table(UnitLang::Pt)),
    }
}

// ---------------------------------------------------------------------------
// Rule logic
// ---------------------------------------------------------------------------

struct RawMatch {
    start: usize,
    end: usize,
    message: &'static str,
    short_message: &'static str,
    suggestions: Vec<String>,
}

impl RawMatch {
    fn into_match(self, cfg: &UnitConversionConfig, base: usize) -> Match {
        Match::new(
            cfg.rule_id,
            Option::<String>::None,
            self.message,
            Some(self.short_message.to_string()),
            TextRange::new(base + self.start, base + self.end),
            self.suggestions
                .into_iter()
                .map(|value| Suggestion {
                    value,
                    short_description: None,
                })
                .collect(),
            "STYLE",
            "Style",
        )
        .with_metadata(cfg.description, "style", 0)
        // `METRIC_UNITS_EN_US` is `tags="picky"`; `EINHEITEN_METRISCH` is a
        // regular default-on rule.
        .with_picky(cfg.lang == UnitLang::EnUs)
    }
}

fn is_ignored(ignore: &[(usize, usize)], start: usize, end: usize) -> bool {
    ignore
        .iter()
        .any(|(range_start, range_end)| start >= *range_start && end <= *range_end)
}

fn special_lookbehind_ok(kind: SpecialKind, text: &str, start: usize) -> bool {
    let before = text[..start].chars().next_back();
    match kind {
        // `(?:(?<=[^º°\d]))`
        SpecialKind::LeadingSpace => {
            before.is_some_and(|c| !matches!(c, 'º' | '°') && !c.is_ascii_digit())
        }
        // `(?:(?<=[^º°\d\s]))`
        SpecialKind::NoLeadingSpace => before
            .is_some_and(|c| !matches!(c, 'º' | '°') && !c.is_ascii_digit() && !c.is_whitespace()),
    }
}

fn detect_number_range(table: &UnitTable, text: &str, matcher_start: usize, group1: &str) -> bool {
    if !group1.starts_with('-') {
        return false;
    }
    table.number_range_part.is_match(&text[..matcher_start])
}

/// `AbstractUnitConversionRule.getMetricEquivalent`.
fn get_metric_equivalent(table: &UnitTable, value: f64, unit: Unit) -> Option<Vec<(Unit, f64)>> {
    let mut conversions: Vec<(Unit, f64)> = Vec::new();
    for metric in &table.metric_units {
        if unit == *metric {
            return None;
        }
        if unit.is_compatible(metric) {
            let converted = unit.convert_to(metric, value)?;
            conversions.push((metric.clone(), converted));
        }
    }
    conversions.sort_by(|a, b| {
        naturalness(a.1)
            .partial_cmp(&naturalness(b.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if conversions.is_empty() {
        None
    } else {
        Some(conversions)
    }
}

/// `AbstractUnitConversionRule.sortByNaturalness`.
fn naturalness(number: f64) -> f64 {
    let abs = number.abs();
    if abs < 1.0 {
        1.0 / (abs * abs * 2.0)
    } else if abs < 100.0 {
        abs - 50.0
    } else {
        abs * abs
    }
}

fn get_formatted_conversions(
    cfg: &UnitConversionConfig,
    table: &UnitTable,
    conversions: &[(Unit, f64)],
) -> Vec<String> {
    let mut formatted: Vec<String> = Vec::new();
    for (unit, converted) in conversions {
        let converted = *converted;
        let rounded = java_round(converted);
        let Some((_, symbols)) = table.symbols.iter().find(|(u, _)| u == unit) else {
            continue;
        };
        for symbol in symbols {
            if formatted.len() > MAX_SUGGESTIONS {
                break;
            }
            if converted.abs() > 0.0
                && (converted - rounded as f64).abs() / converted.abs() < ROUNDING_DELTA
                && rounded != 0
            {
                let formatted_str = format!(
                    "{}{} {}",
                    cfg.rounded_prefix,
                    format_long_locale(cfg.locale, rounded),
                    symbol
                );
                if !formatted.contains(&formatted_str) {
                    formatted.push(formatted_str);
                }
            }
            let formatted_number = format_decimal_locale(cfg.locale, converted);
            let formatted_str = format!("{formatted_number} {symbol}");
            if !formatted.contains(&formatted_str) && formatted_number != "0" {
                formatted.push(formatted_str);
            }
        }
    }
    formatted
}

/// `AbstractUnitConversionRule.formatMeasurement`.
fn format_measurement(
    cfg: &UnitConversionConfig,
    table: &UnitTable,
    value: f64,
    unit: Unit,
) -> Option<Vec<String>> {
    let equivalents = get_metric_equivalent(table, value, unit)?;
    let formatted = get_formatted_conversions(cfg, table, &equivalents);
    if formatted.is_empty() {
        None
    } else {
        Some(formatted)
    }
}

fn suggestion(original: &str, converted: &str) -> String {
    format!("{original} ({converted})")
}

#[allow(clippy::too_many_arguments)]
fn try_conversion(
    cfg: &UnitConversionConfig,
    table: &UnitTable,
    text: &str,
    matches: &mut Vec<RawMatch>,
    ignore: &mut Vec<(usize, usize)>,
    unit: Unit,
    custom_value: Option<f64>,
    m_start: usize,
    m_end: usize,
    group0: &str,
    group1: Option<&str>,
) {
    ignore.push((m_start, m_end));
    let converted_base = m_end;

    // search for an existing conversion, e.g. "5 miles (8km)"
    let mut converted_matcher: Option<regex::Captures> = None;
    if let Some(caps) = table.converted.captures(&text[converted_base..]) {
        if caps.get(0).is_some_and(|m| m.start() == 0) {
            converted_matcher = Some(caps);
        }
    }

    let value = match custom_value {
        Some(value) => value,
        None => {
            let group1 = group1.unwrap_or("");
            let value_str = if detect_number_range(table, text, m_start, group1) {
                &group1[1..]
            } else {
                group1
            };
            match parse_number(cfg.locale, value_str) {
                Some(value) => value,
                None => return,
            }
        }
    };

    let converted = format_measurement(cfg, table, value, unit.clone());
    match (converted.as_ref(), converted_matcher) {
        (None, None) => {}
        (Some(converted), None) => {
            matches.push(RawMatch {
                start: m_start,
                end: m_end,
                message: cfg.message_suggestion,
                short_message: cfg.short_message_suggestion,
                suggestions: converted
                    .iter()
                    .map(|formatted| suggestion(group0, formatted))
                    .collect(),
            });
        }
        (converted, Some(caps)) => {
            let whole = caps.get(0).unwrap();
            let converted_offset = converted_base;
            let converted_range = (
                converted_offset + whole.start(),
                converted_offset + whole.end(),
            );
            ignore.push(converted_range);

            // already using one of our conversions?
            let final_converted_in_text = java_trim(whole.as_str());
            let converted_trimmed = final_converted_in_text
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(final_converted_in_text);
            if let Some(converted) = &converted {
                if converted.iter().any(|s| s == converted_trimmed) {
                    return;
                }
            }
            let converted_unit_pattern = table
                .patterns
                .iter()
                .find(|pattern| pattern.re.is_match(final_converted_in_text));
            let Some(converted_unit_pattern) = converted_unit_pattern else {
                // unknown unit used for conversion
                if let Some(converted) = &converted {
                    let number = caps.get(1).unwrap();
                    let unit_text = caps.get(2).unwrap();
                    matches.push(RawMatch {
                        start: converted_offset + number.start(),
                        end: converted_offset + unit_text.end(),
                        message: cfg.message_check_unknown,
                        short_message: cfg.short_message_check_unknown,
                        suggestions: converted.to_vec(),
                    });
                }
                return;
            };
            let converted_unit = converted_unit_pattern.unit.clone();
            // same unit before and after conversion, e.g. "22.3 cm (20.4 cm)"
            if unit == converted_unit {
                return;
            }
            let Some(converted_value_in_text) =
                parse_number(cfg.locale, caps.get(1).unwrap().as_str())
            else {
                return;
            };
            // e.g. "(2 ft 6 inch)" would be interpreted as just "2 ft"
            if table
                .feet_inch_conversion
                .is_match(java_trim(whole.as_str()))
            {
                return;
            }

            match &converted {
                None => {
                    // already metric: check the conversion in
                    // convertedUnit / convertedValueInText (order may be
                    // reversed)
                    match unit.convert_to(&converted_unit, value) {
                        None => {
                            matches.push(RawMatch {
                                start: m_start,
                                end: converted_offset + whole.end(),
                                message: cfg.message_unit_mismatch,
                                short_message: cfg.short_message_unit_mismatch,
                                suggestions: Vec::new(),
                            });
                        }
                        Some(unit_converted) => {
                            let diff = (unit_converted - converted_value_in_text).abs();
                            if diff > DELTA {
                                let number = caps.get(1).unwrap();
                                let reverse_converted = get_formatted_conversions(
                                    cfg,
                                    table,
                                    &[(converted_unit, unit_converted)],
                                );
                                if reverse_converted.iter().any(|s| s == converted_trimmed) {
                                    return;
                                }
                                matches.push(RawMatch {
                                    start: converted_offset + number.start(),
                                    end: converted_offset + number.end(),
                                    message: cfg.message_check,
                                    short_message: cfg.short_message_check,
                                    suggestions: reverse_converted,
                                });
                            }
                        }
                    }
                }
                Some(converted) => {
                    let Some(metric_equivalents) = get_metric_equivalent(table, value, unit) else {
                        return;
                    };
                    let Some((metric_unit, converted_value_computed)) =
                        metric_equivalents.first().cloned()
                    else {
                        return;
                    };
                    if !(converted_unit == metric_unit
                        && (converted_value_in_text - converted_value_computed).abs() < DELTA)
                    {
                        matches.push(RawMatch {
                            start: m_start,
                            end: converted_offset + whole.end(),
                            message: cfg.message_check,
                            short_message: cfg.short_message_check,
                            suggestions: converted
                                .iter()
                                .map(|formatted| suggestion(group0, formatted))
                                .collect(),
                        });
                    }
                }
            }
        }
    }
}

/// `AbstractUnitConversionRule.match(AnalyzedSentence)`, on the sentence
/// text; returns matches with offsets relative to `text`.
pub fn check_sentence(text: &str, base: usize) -> Vec<Match> {
    check_sentence_cfg(&CONFIG_EN_US, text, base)
}

/// `check_sentence` for a specific locale/variant configuration.
pub fn check_sentence_cfg(cfg: &UnitConversionConfig, text: &str, base: usize) -> Vec<Match> {
    let table = table_for(cfg.lang);
    let mut matches: Vec<RawMatch> = Vec::new();
    let mut ignore: Vec<(usize, usize)> = Vec::new();

    // special patterns where simple number parsing is not enough, e.g. 5'6"
    for (re, kind) in &table.special {
        let mut search = 0usize;
        while let Some(caps) = re.captures_at(text, search) {
            let whole = caps.get(0).unwrap();
            if !special_lookbehind_ok(*kind, text, whole.start()) {
                let next = text[whole.start()..]
                    .chars()
                    .next()
                    .map(|c| whole.start() + c.len_utf8())
                    .unwrap_or(text.len() + 1);
                if next > text.len() {
                    break;
                }
                search = next;
                continue;
            }
            let feet = parse_number(cfg.locale, caps.get(1).unwrap().as_str());
            let inch = caps
                .get(2)
                .and_then(|g| parse_number(cfg.locale, g.as_str()));
            if let Some(feet) = feet {
                let value = feet + inch.unwrap_or(0.0) / 12.0;
                if !is_ignored(&ignore, whole.start(), whole.end()) {
                    try_conversion(
                        cfg,
                        table,
                        text,
                        &mut matches,
                        &mut ignore,
                        table.feet.clone(),
                        Some(value),
                        whole.start(),
                        whole.end(),
                        whole.as_str(),
                        None,
                    );
                }
            }
            search = whole.end();
        }
    }

    // two runs: first metric units, so that ignore ranges are set up
    // properly, then other units
    for is_metric in [true, false] {
        for pattern in &table.patterns {
            if table.metric_units.contains(&pattern.unit) != is_metric {
                continue;
            }
            for caps in pattern.re.captures_iter(text) {
                let whole = caps.get(0).unwrap();
                if pattern.trailing_not_word {
                    // Java `(?!(\\w|\\d))`
                    let next = text[whole.end()..].chars().next();
                    if next.is_some_and(is_word_char) {
                        continue;
                    }
                }
                if is_ignored(&ignore, whole.start(), whole.end()) {
                    continue;
                }
                try_conversion(
                    cfg,
                    table,
                    text,
                    &mut matches,
                    &mut ignore,
                    pattern.unit.clone(),
                    None,
                    whole.start(),
                    whole.end(),
                    whole.as_str(),
                    caps.get(1).map(|g| g.as_str()),
                );
            }
        }
    }

    // deduplicate matches with equal start, longer match wins
    let mut by_start: std::collections::BTreeMap<usize, RawMatch> =
        std::collections::BTreeMap::new();
    for m in matches {
        match by_start.entry(m.start) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(m);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if m.end > entry.get().end {
                    entry.insert(m);
                }
            }
        }
    }
    if !by_start.is_empty() {
        for anti_pattern in &table.anti_patterns {
            let mut pos = 0usize;
            while let Some(m) = anti_pattern.find_at(text, pos) {
                by_start.retain(|_, entry| {
                    !((m.start() <= entry.start && m.end() >= entry.start)
                        || (m.start() <= entry.end && m.end() >= entry.end))
                });
                let next = text[m.end()..]
                    .chars()
                    .next()
                    .map(|c| m.end() + c.len_utf8())
                    .unwrap_or(text.len() + 1);
                if next > text.len() {
                    break;
                }
                pos = next;
            }
        }
    }
    by_start
        .into_values()
        .map(|m| m.into_match(cfg, base))
        .collect()
}

/// Java `\w` (word character) for the `ft`/`inch` lookahead. The `regex`
/// crate's `\w` is `[\p{Alphabetic}\p{M}\p{Nd}\p{Pc}\p{Join_Control}]`, the
/// same set Java uses with `UNICODE_CHARACTER_CLASS` (note `²` is *not* a
/// word character there, unlike Rust's `char::is_alphanumeric`).
fn is_word_char(c: char) -> bool {
    static WORD: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"^\w$").unwrap());
    let mut buf = [0u8; 4];
    WORD.is_match(c.encode_utf8(&mut buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn length_unit(factor: f64) -> Unit {
        Unit::system(Dim::Length).multiply(factor)
    }

    fn mile() -> Unit {
        length_unit(0.3048).multiply(5280.0)
    }

    #[test]
    fn indriya_conversion_chain_matches_java() {
        let kilogram = Unit::system(Dim::Mass);
        let metre = Unit::system(Dim::Length);
        let litre = Unit::system(Dim::Volume).divide(1000.0);
        let us_quart = litre.clone().multiply(0.946352946);
        // Values from the Java `TableDump` probe (LingoTweaker D-021).
        let cases: &[(Unit, Unit, f64)] = &[
            (mile(), metre.clone(), 1609.344),
            (mile(), metre.clone().multiply(1e3), 1.609344),
            (
                length_unit(0.3048).multiply(3.0),
                metre.clone(),
                0.9144000000000001,
            ),
            (length_unit(0.3048), metre.clone(), 0.3048),
            (
                length_unit(0.3048).divide(12.0),
                metre.clone(),
                0.025400000000000002,
            ),
            (
                kilogram.clone().multiply(0.45359237),
                kilogram.clone(),
                0.45359237,
            ),
            (
                kilogram.clone().multiply(0.45359237).divide(12.0),
                kilogram.clone(),
                0.03779936416666667,
            ),
            (us_quart.clone(), litre.clone(), 0.946352946),
            (us_quart.clone().multiply(4.0), litre.clone(), 3.785411784),
            (us_quart.clone().divide(2.0), litre.clone(), 0.473176473),
            (us_quart.clone().divide(4.0), litre.clone(), 0.2365882365),
            (
                us_quart.clone().divide(32.0),
                litre.clone(),
                0.0295735295625,
            ),
            // square/cubic chains
            (
                length_unit(0.3048).times(&length_unit(0.3048)),
                Unit::system(Dim::Area),
                0.09290304,
            ),
            (
                length_unit(0.3048)
                    .divide(12.0)
                    .times(&length_unit(0.3048).divide(12.0)),
                Unit::system(Dim::Area).multiply(1e-4),
                6.451600000000001,
            ),
            (
                length_unit(0.3048)
                    .times(&length_unit(0.3048))
                    .times(&length_unit(0.3048)),
                Unit::system(Dim::Volume),
                0.028316846592000004,
            ),
        ];
        for (from, to, expected) in cases {
            let got = from.convert_to(to, 1.0).unwrap();
            assert_eq!(got, *expected, "{from:?} -> {to:?}: got {got:?}");
        }
        // `KILOMETRE_PER_HOUR` is the hardcoded `0.277778` in Indriya.
        let mph = Unit::quotient(
            &mile(),
            &Unit::system(Dim::Time).multiply(3600.0),
            Dim::Speed,
        );
        let kph = Unit::system(Dim::Speed).multiply(0.277778);
        assert_eq!(mph.convert_to(&kph, 1.0), Some(1.60934271252583));
        assert_eq!(mph.convert_to(&kph, 100.0), Some(160.93427125258302));
        // Fahrenheit chains: (100 °F - 32) * 5/9 = 37.77... °C.
        let kelvin = Unit::system(Dim::Temperature);
        let celsius = kelvin.clone().shift(273.15);
        let fahrenheit = kelvin
            .clone()
            .shift(273.15)
            .multiply(5.0 / 9.0)
            .shift(-32.0);
        assert_eq!(
            fahrenheit.convert_to(&celsius, 100.0),
            Some(37.77777777777778)
        );
        assert_eq!(
            celsius.convert_to(&fahrenheit, 100.0),
            Some(211.99999999999997)
        );
    }

    #[test]
    fn number_parse_matches_java() {
        assert_eq!(parse_number(NumberLocale::EnUs, "1,000"), Some(1000.0));
        assert_eq!(parse_number(NumberLocale::EnUs, "14,080"), Some(14080.0));
        assert_eq!(parse_number(NumberLocale::EnUs, "1,2"), Some(12.0));
        assert_eq!(parse_number(NumberLocale::EnUs, "1.2.3"), Some(1.2));
        assert_eq!(parse_number(NumberLocale::EnUs, "5."), Some(5.0));
        assert_eq!(parse_number(NumberLocale::EnUs, "5,"), Some(5.0));
        assert_eq!(parse_number(NumberLocale::EnUs, "1,,2"), Some(12.0));
        assert_eq!(parse_number(NumberLocale::EnUs, "1.2,3"), Some(1.2));
        assert_eq!(parse_number(NumberLocale::EnUs, "-5.5"), Some(-5.5));
        assert!(parse_number(NumberLocale::EnUs, "-").is_none());
    }

    #[test]
    fn number_format_matches_java() {
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 0.0), "0");
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, -0.0), "-0");
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 1.5), "1.5");
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 2.345), "2.35");
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 2.355), "2.35");
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 2.365), "2.37");
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 0.125), "0.13");
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 0.135), "0.14");
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 0.015), "0.01");
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 0.005), "0.01");
        assert_eq!(
            format_decimal_locale(NumberLocale::EnUs, 160.9344),
            "160.93"
        );
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 99.995), "100");
        assert_eq!(
            format_decimal_locale(NumberLocale::EnUs, 1_000_000.0),
            "1,000,000"
        );
        assert_eq!(
            format_decimal_locale(NumberLocale::EnUs, 453_592.37),
            "453,592.37"
        );
        assert_eq!(
            format_decimal_locale(NumberLocale::EnUs, 1e21),
            "1,000,000,000,000,000,000,000"
        );
        assert_eq!(format_decimal_locale(NumberLocale::EnUs, 1e-7), "0");
        assert_eq!(format_long_locale(NumberLocale::EnUs, 160_934), "160,934");
        assert_eq!(format_long_locale(NumberLocale::EnUs, -8046), "-8,046");
        assert_eq!(java_round(-8.04672), -8);
        assert_eq!(java_round(0.5), 1);
        assert_eq!(java_round(-0.5), 0);
        assert_eq!(java_round(-2.5), -2);
    }
}
