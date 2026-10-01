//! evAI utilities - library of genome/variant annotation analyzers,
//! originally a CLI (by Gaurav Sablok), now driven by an egui desktop UI.

pub mod acmgtranscript;
pub mod altallele;
pub mod altalleleolder;
pub mod annotation;
pub mod annotationolder;
pub mod coordinate;
pub mod coordinateolder;
pub mod coordinatevariant;
pub mod coordinatevariantolder;
pub mod fastanalyzer;
pub mod genomeanalyzer;
pub mod genomedownload;
pub mod gtfanalyzer;
pub mod pathogenicity;
pub mod pathogenicityolder;
pub mod populationvariant;
pub mod populationvariantolder;
pub mod refallele;
pub mod refalleleolder;
pub mod sequenceprofile;
pub mod structfile;
pub mod transcript;
pub mod transcriptolder;
pub mod variantaccumulate;
#[cfg(feature = "database")]
pub mod variantdatabase;
pub mod variantfilter;
