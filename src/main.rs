use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use anyhow::Context;

fn is_buck2_crate(s: &str) -> bool {
    // s.starts_with("buck2_") || s == "cli"
    // true
    s != "superconsole"
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

    fn all_deps_including_self(&self, cr: &str) -> BTreeSet<&'a str> {
        let cr = self.graph.keys().find(|k| **k == cr).unwrap();

        let mut all_deps = BTreeSet::new();
        let mut stack: Vec<&str> = vec![cr];
        while let Some(d) = stack.pop() {
            if all_deps.insert(d) {
                stack.extend(self.first_order_deps(d));
            }
        }
        all_deps
    }

    fn first_order_deps(&self, cr: &str) -> &'a BTreeSet<&'a str> {
        &self.graph.get(cr).unwrap_or_else(|| panic!("{} not found", cr))
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
        self.first_order_deps(cr).into_iter().copied().filter(|d| !reachable.contains(d)).collect()
    }

    fn graph_leading_to(&self, cr: &str) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
        let crates = self.all_deps_including_self(cr);
        let mut graph: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for cr in &crates {
            if !crates.contains(cr) {
                continue;
            }
            let deps = self.first_order_deps(cr).into_iter().copied().filter(|d| crates.contains(d)).collect();
            graph.insert(cr, deps);
        }
        graph
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

fn main() -> anyhow::Result<()> {
    let mut crates: Vec<String> = Vec::new();
    for e in fs::read_dir(".")? {
        let e = e?;
        let dir_name = e.file_name();
        let dir_name = dir_name.to_str().context("UTF-8")?;
        if !is_buck2_crate(dir_name) {
            continue;
        }
        if !Path::new(dir_name).join("Cargo.toml").exists() {
            continue;
        }
        crates.push(dir_name.to_string());
    }
    crates.sort();

    let mut deps_by_crate: BTreeMap<&str, BTreeSet<&str>> = crates.iter().map(|cr| (cr.as_str(), BTreeSet::new())).collect();

    for cr in &crates {
        for line in fs::read_to_string(Path::new(cr).join("Cargo.toml"))?.lines() {
            match line.split_once("=") {
                Some((dep, _)) => {
                    let dep = dep.trim();
                    let dep = crates.iter().find(|c| &dep == c);
                    let dep = match dep {
                        Some(dep) => dep,
                        None => continue,
                    };
                    deps_by_crate.get_mut(cr.as_str()).unwrap().insert(dep);
                }
                None => continue,
            }
        };
    }

    let graph = GraphRef {
        graph: &deps_by_crate,
    };

    let graph = graph.graph_leading_to("cli");
    let graph = GraphRef {
        graph: &graph,
    };

    let graph = graph.min_graph();
    let graph = GraphRef {
        graph: &graph,
    };

    graph.print();

    Ok(())
}
