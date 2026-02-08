//! Nexus Mods API integration

pub mod graphql;
pub mod rest;
pub mod populate;

pub use graphql::{
    NexusClient, ModSearchParams, ModSearchResult, ModSearchPage,
    SortBy, ModFile, DownloadLink, ModUpdateInfo, ModRequirement
};

pub use rest::{NexusRestClient, ModInfo};
pub use populate::{CatalogPopulator, PopulateOptions, PopulateStats};
