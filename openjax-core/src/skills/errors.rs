use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillManifestError {
    #[error("skill frontmatter started but no closing '---' delimiter was found")]
    MissingFrontmatterClosingDelimiter,
    #[error("skill frontmatter yaml parse error: {0}")]
    FrontmatterYaml(String),
}
