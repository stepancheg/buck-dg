use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug)]
struct Crate {
    folder: String,
    name: String,
}

impl Crate {
    fn path(&self) -> PathBuf {
        Path::new(&self.folder).join(&self.name)
    }

    fn cargo_toml(&self) -> PathBuf {
        self.path().join("Cargo.toml")
    }
}

fn is_interesting_crate(s: &str) -> bool {
    // s.starts_with("buck2_") || s == "cli"
    // true
    s != "superconsole" && !s.ends_with("_tests")
}

struct GraphRef<'a> {
    graph: &'a BTreeMap<&'a str, BTreeSet<&'a str>>,
}

impl<'a> GraphRef<'a> {
    /// All deps recursively.
    fn all_deps(&self, cr: &str) -> BTreeSet<&'a str> {
        let mut all_deps = BTreeSet::new();
        let mut stack: Vec<&str> = Vec::from_iter(self.first_order_deps(cr).iter().copied());
        while let Some(d) = stack.pop() {
            if all_deps.insert(d) {
                stack.extend(self.first_order_deps(d));
            }
        }
        all_deps
    }

    fn all_deps_including_self(&self, cr: &str) -> anyhow::Result<BTreeSet<&'a str>> {
        let cr = self
            .graph
            .keys()
            .find(|k| **k == cr)
            .with_context(|| format!("crate not found: {}", cr))?;

        let mut all_deps = BTreeSet::new();
        let mut stack: Vec<&str> = vec![cr];
        while let Some(d) = stack.pop() {
            if all_deps.insert(d) {
                stack.extend(self.first_order_deps(d));
            }
        }
        Ok(all_deps)
    }

    fn first_order_deps(&self, cr: &str) -> &'a BTreeSet<&'a str> {
        &self
            .graph
            .get(cr)
            .unwrap_or_else(|| panic!("{} not found", cr))
    }

    fn deps_reachable_via_first_order_deps(&self, cr: &str) -> BTreeSet<&'a str> {
        let first_order_deps = BTreeSet::from_iter(self.first_order_deps(cr));
        let mut deps = BTreeSet::new();
        for dep in &first_order_deps {
            deps.extend(self.all_deps(dep));
        }
        deps
    }

    fn min_necessary_deps(&self, cr: &str) -> BTreeSet<&'a str> {
        let reachable = self.deps_reachable_via_first_order_deps(cr);
        self.first_order_deps(cr)
            .into_iter()
            .copied()
            .filter(|d| !reachable.contains(d))
            .collect()
    }

    fn graph_leading_to(&self, cr: &str) -> anyhow::Result<BTreeMap<&'a str, BTreeSet<&'a str>>> {
        let crates = self.all_deps_including_self(cr)?;
        let mut graph: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for cr in &crates {
            if !crates.contains(cr) {
                continue;
            }
            let deps = self
                .first_order_deps(cr)
                .into_iter()
                .copied()
                .filter(|d| crates.contains(d))
                .collect();
            graph.insert(cr, deps);
        }
        Ok(graph)
    }

    fn min_graph(&self) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
        let mut min_graph = BTreeMap::new();
        for &cr in self.graph.keys() {
            min_graph.insert(cr, self.min_necessary_deps(cr));
        }
        min_graph
    }

    fn print(&self) {
        for (cr, deps) in self.graph {
            for dep in deps {
                println!("{} -> {}", dep, cr);
            }
        }
    }
}

fn read_dir(path: impl AsRef<Path>) -> anyhow::Result<fs::ReadDir> {
    fs::read_dir(path.as_ref()).with_context(|| format!("read_dir {}", path.as_ref().display()))
}

fn read_to_string(path: impl AsRef<Path>) -> anyhow::Result<String> {
    fs::read_to_string(path.as_ref())
        .with_context(|| format!("read_to_string {}", path.as_ref().display()))
}

fn main() -> anyhow::Result<()> {
    let mut crates: Vec<Crate> = Vec::new();
    for folder in [".", "app"] {
        for e in read_dir(folder)? {
            let e = e?;
            let dir_name = e.file_name();
            let dir_name = dir_name.to_str().context("UTF-8")?;
            if !is_interesting_crate(dir_name) {
                continue;
            }
            if !Path::new(folder).join(dir_name).join("Cargo.toml").exists() {
                continue;
            }
            crates.push(Crate {
                folder: folder.to_owned(),
                name: dir_name.to_string(),
            });
        }
    }
    crates.sort();

    let mut deps_by_crate: BTreeMap<&str, BTreeSet<&str>> = crates
        .iter()
        .map(|cr| (cr.name.as_str(), BTreeSet::new()))
        .collect();

    for cr in &crates {
        for line in read_to_string(cr.cargo_toml())?.lines() {
            match line.split_once("=") {
                Some((dep, _)) => {
                    let dep = dep.trim();
                    let dep = crates.iter().find(|c| dep == c.name);
                    let dep = match dep {
                        Some(dep) => dep,
                        None => continue,
                    };
                    deps_by_crate
                        .get_mut(cr.name.as_str())
                        .unwrap()
                        .insert(&dep.name);
                }
                None => continue,
            }
        }
    }

    let graph = GraphRef {
        graph: &deps_by_crate,
    };

    let graph = graph.graph_leading_to("buck2")?;
    let graph = GraphRef { graph: &graph };

    let graph = graph.min_graph();
    let graph = GraphRef { graph: &graph };

    graph.print();

    Ok(())
}
