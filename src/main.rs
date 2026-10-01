//! evAI Utilities - desktop GUI
//!
//! An egui/eframe front end for the evAI variant & genome annotation
//! toolkit (originally a `clap`-based CLI by Gaurav Sablok). Every
//! subcommand from the original CLI is available here as a form in the
//! sidebar; each run happens on a background thread so the UI never
//! freezes, and results/errors stream into a log panel at the bottom.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui;

/*
Gaurav Sablok
gsablok@proton.me
*/


// ---------------------------------------------------------------------
// Command catalog
// ---------------------------------------------------------------------

/// The kind of input widget a field should render as.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    /// A single file path, with a "Browse..." file picker.
    File,
    /// A directory path, with a "Browse..." folder picker.
    Dir,
    /// A plain text / numeric entry.
    Text,
}

#[derive(Clone, Copy)]
struct FieldSpec {
    key: &'static str,
    label: &'static str,
    kind: FieldKind,
    default: &'static str,
    hint: &'static str,
}

const fn file_field(key: &'static str, label: &'static str, hint: &'static str) -> FieldSpec {
    FieldSpec { key, label, kind: FieldKind::File, default: "", hint }
}
const fn dir_field(key: &'static str, label: &'static str, hint: &'static str) -> FieldSpec {
    FieldSpec { key, label, kind: FieldKind::Dir, default: "", hint }
}
const fn text_field(
    key: &'static str,
    label: &'static str,
    default: &'static str,
    hint: &'static str,
) -> FieldSpec {
    FieldSpec { key, label, kind: FieldKind::Text, default, hint }
}

/// A runnable command: its form fields plus the function that executes it
/// (on a background thread) given the current field values.
struct CommandSpec {
    id: &'static str,
    name: &'static str,
    category: &'static str,
    description: &'static str,
    fields: &'static [FieldSpec],
    run: fn(&HashMap<&'static str, String>) -> Result<String, String>,
}

/// Build a rayon thread pool from a "threads" field and run `f` inside it,
/// mirroring how the original CLI (`main.rs`) executed each subcommand.
fn with_pool<T, F>(threads_str: &str, f: F) -> Result<T, String>
where
    T: Send,
    F: FnOnce() -> Result<T, Box<dyn std::error::Error>> + Send,
{
    let threads: usize = threads_str
        .trim()
        .parse()
        .map_err(|_| format!("'{}' is not a valid number of threads", threads_str))?;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
        .map_err(|e| e.to_string())?;
    // Box<dyn Error> isn't Send, so convert to a Send-safe String *inside*
    // the closure before it crosses the rayon thread-pool boundary.
    pool.install(move || f().map_err(|e| e.to_string()))
}

fn parse_usize(v: &HashMap<&'static str, String>, key: &str) -> Result<usize, String> {
    v.get(key)
        .map(|s| s.trim())
        .unwrap_or("")
        .parse::<usize>()
        .map_err(|_| format!("field '{}' must be a whole number", key))
}

fn parse_f32(v: &HashMap<&'static str, String>, key: &str) -> Result<f32, String> {
    v.get(key)
        .map(|s| s.trim())
        .unwrap_or("")
        .parse::<f32>()
        .map_err(|_| format!("field '{}' must be a number", key))
}

fn get(v: &HashMap<&'static str, String>, key: &str) -> String {
    v.get(key).cloned().unwrap_or_default()
}

// --- run functions, one per CLI subcommand -----------------------------

fn run_variant_analyzer(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgfile = get(v, "acmgfile");
    let tsvfile = get(v, "tsvfile");
    with_pool(&get(v, "threads"), move || {
        evai_gui::genomeanalyzer::genomemap(&acmgfile, &tsvfile)
    })
}

fn run_variant_filter(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgfile = get(v, "acmgfile");
    let tsvfile = get(v, "tsvfile");
    let variant = get(v, "variant");
    with_pool(&get(v, "threads"), move || {
        evai_gui::variantfilter::variantanalyzer(&acmgfile, &tsvfile, &variant)
    })
}

fn run_variant_database(v: &HashMap<&'static str, String>) -> Result<String, String> {
    #[cfg(feature = "database")]
    {
        let acmgfile = get(v, "acmgfile");
        let tsvfile = get(v, "tsvfile");
        let databaseurl = get(v, "databaseurl");
        evai_gui::variantdatabase::variantdatabase(&acmgfile, &tsvfile, &databaseurl)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(feature = "database"))]
    {
        let _ = v;
        Err("This build was compiled without the optional `database` feature (it needs \
             sqlx + tokio, which need a recent Rust toolchain). Uncomment the sqlx/tokio \
             dependencies in Cargo.toml, re-enable `mod variantdatabase;` in src/lib.rs, \
             build with `cargo build --features database`, and set the environment \
             variable named in the 'database url' field to your Postgres connection string."
            .to_string())
    }
}

fn run_gtf_analyze(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let gtffile = get(v, "gtffile");
    with_pool(&get(v, "threads"), move || evai_gui::gtfanalyzer::analyzegtf(&gtffile))
}

fn run_variant_seq(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgfile = get(v, "acmgfile");
    let fastafile = get(v, "fastafile");
    with_pool(&get(v, "threads"), move || {
        evai_gui::fastanalyzer::fastagtf(&acmgfile, &fastafile)
    })
}

fn run_download_genome(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let input = get(v, "input");
    evai_gui::genomedownload::downloadgenome(&input).map_err(|e| e.to_string())
}

fn run_acmg_transcript(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgfile = get(v, "acmgfile");
    with_pool(&get(v, "threads"), move || evai_gui::acmgtranscript::acmgannotate(&acmgfile))
}

fn run_sequence_profile(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgfile = get(v, "acmgfile");
    let fastafile = get(v, "fastafile");
    let upstream = parse_usize(v, "upstream")?;
    let downstream = parse_usize(v, "downstream")?;
    let variant = get(v, "variant");
    with_pool(&get(v, "threads"), move || {
        evai_gui::sequenceprofile::sequence(&acmgfile, &fastafile, upstream, downstream, &variant)
    })
}

fn run_variant_plotter(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let pathfolder = get(v, "pathfolder");
    with_pool(&get(v, "threads"), move || {
        evai_gui::variantaccumulate::variantaccumulateplot(&pathfolder)
    })
}

fn run_population(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let variant = get(v, "variant");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || evai_gui::populationvariant::population(&acmgdir, &variant, name))
}

fn run_population_older(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let variant = get(v, "variant");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::populationvariantolder::populationolder(&acmgdir, &variant, name)
    })
}

fn run_coordinate_search(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let start = parse_usize(v, "start")?;
    let end = parse_usize(v, "end")?;
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::coordinate::coordinatesearch(&acmgdir, start, end, &name)
    })
}

fn run_coordinate_search_older(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let start = parse_usize(v, "start")?;
    let end = parse_usize(v, "end")?;
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::coordinateolder::coordinatesearcholder(&acmgdir, start, end, &name)
    })
}

fn run_coordinate_variant_search(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let start = parse_usize(v, "start")?;
    let end = parse_usize(v, "end")?;
    let variant = get(v, "variant");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::coordinatevariant::coordinatevariantsearch(&acmgdir, start, end, &variant, &name)
    })
}

fn run_coordinate_variant_search_older(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let start = parse_usize(v, "start")?;
    let end = parse_usize(v, "end")?;
    let variant = get(v, "variant");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::coordinatevariantolder::coordinatevariantsearcholder(&acmgdir, start, end, &variant, &name)
    })
}

fn run_annotation_search(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let genename = get(v, "genename");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::annotation::annotationsearch(&acmgdir, &genename, &name)
    })
}

fn run_annotation_search_older(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let genename = get(v, "genename");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::annotationolder::annotationsearcholder(&acmgdir, &genename, &name)
    })
}

fn run_pathogenicity_filter(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let start = parse_f32(v, "start")?;
    let end = parse_f32(v, "end")?;
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::pathogenicity::pathogenicityscore(&acmgdir, start, end, &name)
    })
}

fn run_pathogenicity_filter_older(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let start = parse_f32(v, "start")?;
    let end = parse_f32(v, "end")?;
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::pathogenicityolder::pathogenicityscoreolder(&acmgdir, start, end, &name)
    })
}

fn run_transcript_search(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let transcript = get(v, "transcript");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::transcript::transcriptsearch(&acmgdir, &transcript, &name)
    })
}

fn run_transcript_search_older(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let transcript = get(v, "transcript");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::transcriptolder::transcriptsearcholder(&acmgdir, &transcript, &name)
    })
}

fn run_alt_allele(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let refallele = get(v, "refallele");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || evai_gui::refallele::altallelesearch(&acmgdir, &refallele, name))
}

fn run_alt_allele_older(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let refallele = get(v, "refallele");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::refalleleolder::altalleleoldersearch(&acmgdir, &refallele, name)
    })
}

fn run_alt_ref_allele(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let refallele = get(v, "refallele");
    let altallele = get(v, "altallele");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::altallele::altrefallelesearch(&acmgdir, &refallele, &altallele, name)
    })
}

fn run_alt_ref_allele_older(v: &HashMap<&'static str, String>) -> Result<String, String> {
    let acmgdir = get(v, "acmgdir");
    let refallele = get(v, "refallele");
    let altallele = get(v, "altallele");
    let name = get(v, "name");
    with_pool(&get(v, "threads"), move || {
        evai_gui::altalleleolder::altrefalleleoldersearch(&acmgdir, &refallele, &altallele, name)
    })
}

// --- field lists ---------------------------------------------------------

const F_VARIANT_ANALYZER: &[FieldSpec] = &[
    file_field("acmgfile", "ACMG file", "The eVai ACMG result file to convert"),
    file_field("tsvfile", "TSV file", "Companion TSV file"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_VARIANT_FILTER: &[FieldSpec] = &[
    file_field("acmgfile", "ACMG file", "The eVai ACMG result file"),
    file_field("tsvfile", "TSV file", "Companion TSV file"),
    text_field("variant", "Variant", "SNV", "Variant type to filter for"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_VARIANT_DATABASE: &[FieldSpec] = &[
    file_field("acmgfile", "ACMG file", "The eVai ACMG result file"),
    file_field("tsvfile", "TSV file", "Companion TSV file"),
    text_field(
        "databaseurl",
        "Database URL env var",
        "DATABASE_URL",
        "Name of the environment variable holding the Postgres connection string",
    ),
];
const F_GTF_ANALYZE: &[FieldSpec] = &[
    file_field("gtffile", "GTF file", "Genome annotation file to analyze"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_VARIANT_SEQ: &[FieldSpec] = &[
    file_field("acmgfile", "ACMG file", "The eVai ACMG result file"),
    file_field("fastafile", "FASTA file", "Reference sequence file"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_DOWNLOAD_GENOME: &[FieldSpec] = &[text_field(
    "input",
    "Confirm download",
    "no",
    "Type 'yes' to download the full GRCh38 + GENCODE reference set (large, slow)",
)];
const F_ACMG_TRANSCRIPT: &[FieldSpec] = &[
    file_field("acmgfile", "ACMG file", "The eVai ACMG result file"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_SEQUENCE_PROFILE: &[FieldSpec] = &[
    file_field("acmgfile", "ACMG file", "The eVai ACMG result file"),
    file_field("fastafile", "FASTA file", "Reference sequence file"),
    text_field("upstream", "Upstream (bp)", "50", "Bases upstream of the variant to capture"),
    text_field("downstream", "Downstream (bp)", "50", "Bases downstream of the variant to capture"),
    text_field("variant", "Variant", "SNV", "Variant type"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_VARIANT_PLOTTER: &[FieldSpec] = &[
    dir_field("pathfolder", "Results folder", "Folder of ACMG result files to accumulate for plotting"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_SEARCH_VARIANT: &[FieldSpec] = &[
    dir_field("acmgdir", "ACMG directory", "Directory of ACMG result files to search"),
    text_field("variant", "Variant", "SNV", "Variant to search for"),
    text_field("name", "Analysis name", "analysis1", "Name/label for this run's output"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_SEARCH_COORDINATE: &[FieldSpec] = &[
    dir_field("acmgdir", "ACMG directory", "Directory of ACMG result files to search"),
    text_field("start", "Start", "0", "Start coordinate"),
    text_field("end", "End", "0", "End coordinate"),
    text_field("name", "Analysis name", "analysis1", "Name/label for this run's output"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_SEARCH_COORDINATE_VARIANT: &[FieldSpec] = &[
    dir_field("acmgdir", "ACMG directory", "Directory of ACMG result files to search"),
    text_field("start", "Start", "0", "Start coordinate"),
    text_field("end", "End", "0", "End coordinate"),
    text_field("variant", "Variant", "SNV", "Variant type"),
    text_field("name", "Analysis name", "analysis1", "Name/label for this run's output"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_SEARCH_ANNOTATION: &[FieldSpec] = &[
    dir_field("acmgdir", "ACMG directory", "Directory of ACMG result files to search"),
    text_field("genename", "Gene name", "TP53", "Gene symbol to search for"),
    text_field("name", "Analysis name", "analysis1", "Name/label for this run's output"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_PATHOGENICITY: &[FieldSpec] = &[
    dir_field("acmgdir", "ACMG directory", "Directory of ACMG result files to search"),
    text_field("start", "Min score", "0.0", "Lower pathogenicity score bound"),
    text_field("end", "Max score", "1.0", "Upper pathogenicity score bound"),
    text_field("name", "Analysis name", "analysis1", "Name/label for this run's output"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_SEARCH_TRANSCRIPT: &[FieldSpec] = &[
    dir_field("acmgdir", "ACMG directory", "Directory of ACMG result files to search"),
    text_field("transcript", "Transcript ID", "NM_000000", "Transcript identifier to search for"),
    text_field("name", "Analysis name", "analysis1", "Name/label for this run's output"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_ALT_ALLELE: &[FieldSpec] = &[
    dir_field("acmgdir", "ACMG directory", "Directory of ACMG result files to search"),
    text_field("refallele", "Ref allele", "A", "Reference allele to search for"),
    text_field("name", "Analysis name", "analysis1", "Name/label for this run's output"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];
const F_ALT_REF_ALLELE: &[FieldSpec] = &[
    dir_field("acmgdir", "ACMG directory", "Directory of ACMG result files to search"),
    text_field("refallele", "Ref allele", "A", "Reference allele"),
    text_field("altallele", "Alt allele", "T", "Alternate allele"),
    text_field("name", "Analysis name", "analysis1", "Name/label for this run's output"),
    text_field("threads", "Threads", "4", "Worker threads for the analysis"),
];

const CAT_PIPELINE: &str = "Pipeline";
const CAT_CURRENT: &str = "Search & filter";
const CAT_LEGACY: &str = "Search & filter (legacy)";

static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "variant_analyzer",
        name: "Variant Analyzer",
        category: CAT_PIPELINE,
        description: "Convert an eVai ACMG + TSV pair into the analyzer's internal format.",
        fields: F_VARIANT_ANALYZER,
        run: run_variant_analyzer,
    },
    CommandSpec {
        id: "gtf_analyze",
        name: "GTF Analyze",
        category: CAT_PIPELINE,
        description: "Parse and summarize a GTF genome annotation file.",
        fields: F_GTF_ANALYZE,
        run: run_gtf_analyze,
    },
    CommandSpec {
        id: "variant_seq",
        name: "Variant Sequence",
        category: CAT_PIPELINE,
        description: "Prepare sequence annotation for variants using a FASTA reference.",
        fields: F_VARIANT_SEQ,
        run: run_variant_seq,
    },
    CommandSpec {
        id: "acmg_transcript",
        name: "ACMG Transcript IDs",
        category: CAT_PIPELINE,
        description: "Extract variant-to-transcript ID mappings from an ACMG file.",
        fields: F_ACMG_TRANSCRIPT,
        run: run_acmg_transcript,
    },
    CommandSpec {
        id: "sequence_profile",
        name: "Sequence Profile",
        category: CAT_PIPELINE,
        description: "Capture upstream/downstream sequence context around a variant.",
        fields: F_SEQUENCE_PROFILE,
        run: run_sequence_profile,
    },
    CommandSpec {
        id: "variant_filter",
        name: "Variant Filter",
        category: CAT_PIPELINE,
        description: "Filter an ACMG/TSV pair down to a specific variant type.",
        fields: F_VARIANT_FILTER,
        run: run_variant_filter,
    },
    CommandSpec {
        id: "variant_plotter",
        name: "Variant Plotter",
        category: CAT_PIPELINE,
        description: "Accumulate variants across a folder of results for plotting.",
        fields: F_VARIANT_PLOTTER,
        run: run_variant_plotter,
    },
    CommandSpec {
        id: "download_genome",
        name: "Download Genome",
        category: CAT_PIPELINE,
        description: "Download the GRCh38 reference genome + GENCODE annotations (large).",
        fields: F_DOWNLOAD_GENOME,
        run: run_download_genome,
    },
    CommandSpec {
        id: "variant_database",
        name: "Variant Database",
        category: CAT_PIPELINE,
        description: "Load an ACMG/TSV pair into a PostgreSQL variant database. \
                       Requires building with `--features database` (see the field hints).",
        fields: F_VARIANT_DATABASE,
        run: run_variant_database,
    },
    CommandSpec {
        id: "population_search",
        name: "Population Variant Search",
        category: CAT_CURRENT,
        description: "Search a directory of ACMG results for a variant across the population.",
        fields: F_SEARCH_VARIANT,
        run: run_population,
    },
    CommandSpec {
        id: "coordinate_search",
        name: "Coordinate Search",
        category: CAT_CURRENT,
        description: "Search ACMG results within a coordinate range.",
        fields: F_SEARCH_COORDINATE,
        run: run_coordinate_search,
    },
    CommandSpec {
        id: "coordinate_variant_search",
        name: "Coordinate + Variant Search",
        category: CAT_CURRENT,
        description: "Search ACMG results by coordinate range and variant type.",
        fields: F_SEARCH_COORDINATE_VARIANT,
        run: run_coordinate_variant_search,
    },
    CommandSpec {
        id: "annotation_search",
        name: "Annotation Search",
        category: CAT_CURRENT,
        description: "Search ACMG results by gene name / annotation.",
        fields: F_SEARCH_ANNOTATION,
        run: run_annotation_search,
    },
    CommandSpec {
        id: "pathogenicity_filter",
        name: "Pathogenicity Filter",
        category: CAT_CURRENT,
        description: "Filter ACMG results by a pathogenicity score range.",
        fields: F_PATHOGENICITY,
        run: run_pathogenicity_filter,
    },
    CommandSpec {
        id: "transcript_search",
        name: "Transcript Search",
        category: CAT_CURRENT,
        description: "Search ACMG results by transcript identifier.",
        fields: F_SEARCH_TRANSCRIPT,
        run: run_transcript_search,
    },
    CommandSpec {
        id: "alt_allele",
        name: "Alt Allele Search",
        category: CAT_CURRENT,
        description: "Search ACMG results by reference allele.",
        fields: F_ALT_ALLELE,
        run: run_alt_allele,
    },
    CommandSpec {
        id: "alt_ref_allele",
        name: "Ref + Alt Allele Search",
        category: CAT_CURRENT,
        description: "Search ACMG results by reference and alternate allele pair.",
        fields: F_ALT_REF_ALLELE,
        run: run_alt_ref_allele,
    },
    CommandSpec {
        id: "population_search_older",
        name: "Population Variant Search",
        category: CAT_LEGACY,
        description: "Legacy-format version of the population variant search.",
        fields: F_SEARCH_VARIANT,
        run: run_population_older,
    },
    CommandSpec {
        id: "coordinate_search_older",
        name: "Coordinate Search",
        category: CAT_LEGACY,
        description: "Legacy-format version of the coordinate search.",
        fields: F_SEARCH_COORDINATE,
        run: run_coordinate_search_older,
    },
    CommandSpec {
        id: "coordinate_variant_search_older",
        name: "Coordinate + Variant Search",
        category: CAT_LEGACY,
        description: "Legacy-format version of the coordinate + variant search.",
        fields: F_SEARCH_COORDINATE_VARIANT,
        run: run_coordinate_variant_search_older,
    },
    CommandSpec {
        id: "annotation_search_older",
        name: "Annotation Search",
        category: CAT_LEGACY,
        description: "Legacy-format version of the annotation search.",
        fields: F_SEARCH_ANNOTATION,
        run: run_annotation_search_older,
    },
    CommandSpec {
        id: "pathogenicity_filter_older",
        name: "Pathogenicity Filter",
        category: CAT_LEGACY,
        description: "Legacy-format version of the pathogenicity filter.",
        fields: F_PATHOGENICITY,
        run: run_pathogenicity_filter_older,
    },
    CommandSpec {
        id: "transcript_search_older",
        name: "Transcript Search",
        category: CAT_LEGACY,
        description: "Legacy-format version of the transcript search.",
        fields: F_SEARCH_TRANSCRIPT,
        run: run_transcript_search_older,
    },
    CommandSpec {
        id: "alt_allele_older",
        name: "Alt Allele Search",
        category: CAT_LEGACY,
        description: "Legacy-format version of the alt allele search.",
        fields: F_ALT_ALLELE,
        run: run_alt_allele_older,
    },
    CommandSpec {
        id: "alt_ref_allele_older",
        name: "Ref + Alt Allele Search",
        category: CAT_LEGACY,
        description: "Legacy-format version of the ref + alt allele search.",
        fields: F_ALT_REF_ALLELE,
        run: run_alt_ref_allele_older,
    },
];

// ---------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------

struct LogEntry {
    time: String,
    command: &'static str,
    ok: bool,
    message: String,
    elapsed: Duration,
}

struct JobResult {
    command: &'static str,
    ok: bool,
    message: String,
    elapsed: Duration,
}

struct EvaiApp {
    selected: usize,
    /// Per-command field values, keyed by command id.
    values: HashMap<&'static str, HashMap<&'static str, String>>,
    log: Vec<LogEntry>,
    running: bool,
    running_since: Option<Instant>,
    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
}

impl EvaiApp {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut values = HashMap::new();
        for spec in COMMANDS {
            let mut m = HashMap::new();
            for f in spec.fields {
                m.insert(f.key, f.default.to_string());
            }
            values.insert(spec.id, m);
        }
        Self {
            selected: 0,
            values,
            log: Vec::new(),
            running: false,
            running_since: None,
            tx,
            rx,
        }
    }

    fn run_selected(&mut self) {
        if self.running {
            return;
        }
        let spec = &COMMANDS[self.selected];
        let values = self.values.get(spec.id).cloned().unwrap_or_default();
        let tx = self.tx.clone();
        let id = spec.id;
        let run_fn = spec.run;
        self.running = true;
        self.running_since = Some(Instant::now());
        std::thread::spawn(move || {
            let start = Instant::now();
            let result = run_fn(&values);
            let elapsed = start.elapsed();
            let job = match result {
                Ok(message) => JobResult { command: id, ok: true, message, elapsed },
                Err(message) => JobResult { command: id, ok: false, message, elapsed },
            };
            // The receiver may occasionally have been dropped (app closing).
            let _ = tx.send(job);
        });
    }

    fn poll_jobs(&mut self) {
        while let Ok(job) = self.rx.try_recv() {
            self.running = false;
            self.running_since = None;
            self.log.push(LogEntry {
                time: clock_timestamp(),
                command: job.command,
                ok: job.ok,
                message: job.message,
                elapsed: job.elapsed,
            });
        }
    }
}

/// A tiny dependency-free "HH:MM:SS" wall-clock timestamp for the log,
/// so we don't need to pull in the `chrono` or `time` crates.
fn clock_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs_today = now % 86_400;
    format!("{:02}:{:02}:{:02}", secs_today / 3600, (secs_today % 3600) / 60, secs_today % 60)
}

fn command_name(id: &str) -> &'static str {
    COMMANDS.iter().find(|c| c.id == id).map(|c| c.name).unwrap_or("unknown")
}

impl eframe::App for EvaiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_jobs();
        if self.running {
            // Keep repainting so we notice when the background job finishes
            // and so the "Running..." spinner animates.
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("evAI Utilities");
                ui.label(egui::RichText::new("variant & genome annotation toolkit").weak());
            });
            ui.add_space(6.0);
        });

        egui::SidePanel::left("sidebar").resizable(true).default_width(260.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for category in [CAT_PIPELINE, CAT_CURRENT, CAT_LEGACY] {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(category).strong().underline());
                    ui.add_space(4.0);
                    for (idx, spec) in COMMANDS.iter().enumerate() {
                        if spec.category != category {
                            continue;
                        }
                        let selected = idx == self.selected;
                        if ui.selectable_label(selected, spec.name).clicked() {
                            self.selected = idx;
                        }
                    }
                }
            });
        });

        egui::TopBottomPanel::bottom("log_panel")
            .resizable(true)
            .default_height(220.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Output log").strong());
                    if ui.small_button("Clear").clicked() {
                        self.log.clear();
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    if self.log.is_empty() {
                        ui.label(egui::RichText::new("Runs will appear here.").weak());
                    }
                    for entry in &self.log {
                        let (icon, color) = if entry.ok {
                            ("done", egui::Color32::from_rgb(90, 200, 90))
                        } else {
                            ("error", egui::Color32::from_rgb(220, 90, 90))
                        };
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(icon).color(color).monospace());
                            ui.label(
                                egui::RichText::new(format!(
                                    "[{}] {} ({:.2?})",
                                    entry.time,
                                    command_name(entry.command),
                                    entry.elapsed
                                ))
                                .strong(),
                            );
                        });
                        ui.label(&entry.message);
                        ui.add_space(6.0);
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let spec_idx = self.selected;
            let spec_id = COMMANDS[spec_idx].id;
            let spec_name = COMMANDS[spec_idx].name;
            let spec_desc = COMMANDS[spec_idx].description;
            let spec_fields = COMMANDS[spec_idx].fields;

            ui.heading(spec_name);
            ui.label(egui::RichText::new(spec_desc).weak());
            ui.add_space(12.0);

            egui::Grid::new("field_grid").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                let values = self.values.entry(spec_id).or_default();
                for field in spec_fields {
                    ui.label(field.label).on_hover_text(field.hint);
                    let entry = values.entry(field.key).or_insert_with(|| field.default.to_string());
                    match field.kind {
                        FieldKind::Text => {
                            ui.add(egui::TextEdit::singleline(entry).hint_text(field.hint));
                        }
                        FieldKind::File | FieldKind::Dir => {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(entry)
                                        .hint_text(field.hint)
                                        .desired_width(ui.available_width() - 90.0),
                                );
                                if ui.button("Browse...").clicked() {
                                    let picked = if field.kind == FieldKind::File {
                                        rfd::FileDialog::new().pick_file()
                                    } else {
                                        rfd::FileDialog::new().pick_folder()
                                    };
                                    if let Some(path) = picked {
                                        *entry = path.to_string_lossy().to_string();
                                    }
                                }
                            });
                        }
                    }
                    ui.end_row();
                }
            });

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                let run_button = egui::Button::new(if self.running { "Running..." } else { "Run" });
                if ui.add_enabled(!self.running, run_button).clicked() {
                    self.run_selected();
                }
                if self.running {
                    ui.spinner();
                    if let Some(since) = self.running_since {
                        ui.label(format!("{:.1}s", since.elapsed().as_secs_f32()));
                    }
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1080.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "evAI Utilities",
        options,
        Box::new(|cc| {
            let mut visuals = egui::Visuals::dark();
            visuals.selection.bg_fill = egui::Color32::from_rgb(255, 100, 20);
            cc.egui_ctx.set_visuals(visuals);
            Box::new(EvaiApp::new())
        }),
    )
}
