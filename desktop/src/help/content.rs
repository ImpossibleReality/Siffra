#[derive(Clone, Copy)]
pub struct OverviewCategory {
    pub name: &'static str,
    pub entries: &'static [OverviewEntry],
}

#[derive(Clone, Copy)]
pub struct OverviewEntry {
    pub title: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    pub examples: &'static [ExampleLine],
}

#[derive(Clone, Copy)]
pub struct ExampleLine {
    pub input: &'static str,
    pub result: Option<&'static str>,
}

macro_rules! line {
    ($input:literal) => {
        ExampleLine {
            input: $input,
            result: None,
        }
    };
    ($input:literal => $result:literal) => {
        ExampleLine {
            input: $input,
            result: Some($result),
        }
    };
}

macro_rules! category {
    (
        $category:literal => [
            $(
                $title:literal {
                    description: $description:literal,
                    keywords: [$($keyword:literal),* $(,)?],
                    examples: [$($input:literal $(=> $result:literal)?),* $(,)?],
                }
            ),* $(,)?
        ]
    ) => {
        OverviewCategory {
            name: $category,
            entries: &[
                $(
                    OverviewEntry {
                        title: $title,
                        description: $description,
                        keywords: &[$($keyword),*],
                        examples: &[$(line!($input $(=> $result)?)),*],
                    }
                ),*
            ],
        }
    };
}

pub const OVERVIEW: &[OverviewCategory] = &[
    category!("Arithmetic" => [
        "Addition & subtraction" {
            description: "Add and subtract with the usual symbols, or spell them out as “plus” and “minus”.",
            keywords: ["add", "subtract", "plus", "minus", "sum", "+", "-"],
            examples: ["2 + 3" => "5", "10 minus 4" => "6"],
        },
        "Multiplication & division" {
            description: "Multiply and divide with *, /, or words like “times”, “over”, and “divided by”.",
            keywords: [
                "multiply",
                "divide",
                "times",
                "over",
                "divided by",
                "product",
                "*",
                "/",
            ],
            examples: [
                "6 * 7" => "42",
                "20 over 4" => "5",
                "100 divided by 8" => "12.5",
            ],
        },
        "Powers & grouping" {
            description: "Raise to a power with ^, and group sub-expressions with parentheses.",
            keywords: ["power", "exponent", "parentheses", "brackets", "^"],
            examples: ["2 ^ 10" => "1,024", "(3 + 4) * 2" => "14"],
        },
        "Variables" {
            description: "Name a value with “=” and reuse it on later lines.",
            keywords: ["variable", "assign", "let", "name", "="],
            examples: ["price = 40" => "40", "price * 3" => "120"],
        },
    ]),
    category!("Conversions" => [
        "Convert between units" {
            description: "Use “in”, “to”, or “as” to convert a value into another compatible unit.",
            keywords: ["convert", "in", "to", "as", "into", "unit"],
            examples: [
                "5 km in miles" => "3.106856 mi",
                "2 hours in minutes" => "120 min",
            ],
        },
        "Temperature & data" {
            description: "Conversions work across every dimension — temperature scales, data sizes, currencies, and more.",
            keywords: ["temperature", "celsius", "fahrenheit", "data", "gb", "mb"],
            examples: [
                "100 F in C" => "37.777778 °C",
                "1 GB in MB" => "1,000 MB",
            ],
        },
        "Unit analysis" {
            description: "Multiply, divide, and raise units to powers — Siffra tracks the dimensions and keeps the answer's units correct.",
            keywords: [
                "dimensional",
                "dimension",
                "analysis",
                "area",
                "volume",
                "per",
                "compound",
                "bracket",
                "m^2",
            ],
            examples: [
                "10 m * 5 m" => "50 m^2",
                "100 km / 2 h" => "50 km/h",
                "(3 m)^2" => "9 m^2",
                "5 N * 2 m" => "10 N*m",
                "5 [kg * m / s^2] in N" => "5 N",
            ],
        },
    ]),
    category!("Percentages" => [
        "Take, add, or remove a percentage" {
            description: "“of” takes a percentage; “on” adds it; “off” removes it. Works with any unit or currency.",
            keywords: ["percent", "of", "on", "off", "discount", "tax", "tip", "%"],
            examples: [
                "20% of $10" => "2 USD",
                "5% on $30" => "31.5 USD",
                "6% off 40 EUR" => "37.6 EUR",
            ],
        },
        "One value as a percentage of another" {
            description: "“X as a % of / on / off Y” expresses X relative to Y as a percentage.",
            keywords: ["percent", "relative", "as a", "ratio", "share", "%"],
            examples: [
                "$50 as a % of $100" => "50 %",
                "$70 as a % on $20" => "250 %",
                "$20 as a % off $70" => "71.428571 %",
            ],
        },
        "Find the value behind a percentage" {
            description: "“X% of / on / off what is Y” works backwards from a percentage and a result to the original value.",
            keywords: ["percent", "reverse", "what", "original", "backwards", "%"],
            examples: [
                "5% of what is 6 EUR" => "120 EUR",
                "5% on what is 6 EUR" => "5.714286 EUR",
                "5% off what is 6 EUR" => "6.315789 EUR",
            ],
        },
    ]),
    category!("Molar Mass Conversions" => [
        "Grams to moles of a compound" {
            description: "Tag a mass with a chemical formula and convert it to an amount of substance — Siffra looks up the molar mass from the periodic table.",
            keywords: [
                "stoichiometry",
                "chemistry",
                "compound",
                "formula",
                "molar mass",
                "molecular weight",
                "mole",
                "mol",
                "O2",
                "NaCl",
            ],
            examples: [
                "5 g O2 in mol O2" => "0.15626 mol O2",
                "10 g NaCl in mol NaCl" => "0.171116 mol NaCl",
            ],
        },
        "Moles to grams of a compound" {
            description: "Go the other way to get a mass. Parentheses and subscripts in formulas like Ca(NO3)2 are understood.",
            keywords: [
                "stoichiometry",
                "chemistry",
                "compound",
                "formula",
                "molar mass",
                "mole",
                "mol",
                "grams",
                "Ca(NO3)2",
                "H2O",
            ],
            examples: [
                "5 mol Ca(NO3)2 into g" => "820.44 g",
                "2 mol H2O in g" => "36.03 g",
            ],
        },
        "Bracketed compound units" {
            description: "Wrap a compound unit in [ ] when you want spaces or need it inside a larger expression.",
            keywords: [
                "stoichiometry",
                "chemistry",
                "compound",
                "brackets",
                "molar",
                "H2O",
            ],
            examples: ["18 [g H2O] as [mol H2O]" => "0.999167 mol H2O"],
        },
    ]),
    category!("Aggregates & recall" => [
        "Sum a column" {
            description: "“total” (or “sum”) adds up the results above it, back to the last blank line.",
            keywords: ["total", "sum", "column", "add", "aggregate"],
            examples: [
                "10" => "10",
                "20" => "20",
                "30" => "30",
                "total" => "60",
            ],
        },
        "Reuse the previous answer" {
            description: "“prev” (also “ans”, “previous”, or “that”) refers to the result on the line above.",
            keywords: ["prev", "ans", "previous", "that", "recall", "answer"],
            examples: ["5 * 5" => "25", "prev + 10" => "35"],
        },
    ]),
    category!("Naming & notes" => [
        "Headings" {
            description: "Start any line with # to make it a heading. Headings are shown larger and produce no result, so they can title a calculation or break a long one into sections. To name the file itself, click its name in the title bar.",
            keywords: [
                "header", "heading", "title", "section", "name", "file", "rename", "#",
            ],
            examples: [
                "# Trip budget",
                "flights = 450 USD" => "450 USD",
                "## Per person",
                "flights / 2" => "225 USD",
            ],
        },
        "Comments" {
            description: "Anything after // is ignored, as is a /* … */ block, so you can annotate a line without changing its result.",
            keywords: ["comment", "note", "ignore", "//", "/*"],
            examples: [
                "60 * 8 // hours in a work week" => "480",
                "/* scratch space */",
            ],
        },
    ]),
];
