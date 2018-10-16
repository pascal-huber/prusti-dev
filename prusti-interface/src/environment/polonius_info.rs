// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use rustc::hir::def_id::DefId;
use rustc::hir;
use rustc::mir;
use rustc::ty;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use super::loops;
use super::borrowck::{facts, regions};
use std::fs::File;
use polonius_engine::{Algorithm, Output, Atom};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ExpiringBorrow<'tcx> {
    pub expiring: mir::Place<'tcx>,
    pub restored: mir::Rvalue<'tcx>,
    pub location: mir::Location,
}

pub struct LoopMagicWand {
    /// Basic block that is the loop head.
    loop_id: mir::BasicBlock,
    /// The reference on the left hand side of the magic wand.
    variable: mir::Local,
    /// The region of the reference.
    region: facts::Region,
    /// Loans that are kept alive by this reference.
    loans: Vec<facts::Loan>,
}

impl fmt::Debug for LoopMagicWand {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({:?}:{:?} --* ", self.variable, self.region)?;
        for loan in self.loans.iter() {
            write!(f, "{:?},", loan)?;
        }
        write!(f, ")")
    }
}

pub enum ReborrowingKind {
    Assignment {
        /// The actual loan that expired.
        loan: facts::Loan,
    },
    Call {
        /// The actual loan that expired.
        loan: facts::Loan,
        /// MIR local variable used as a target to which the result was assigned.
        variable: mir::Local,
        /// The region of the MIR local variable.
        region: facts::Region,
    },
    Loop,
}

pub enum ReborrowingBranching {
    /// This node is a leaf node.
    Leaf,
    /// This node has a single child, no branch is needed.
    Single {
        child: Box<ReborrowingNode>,
    },
    /// This node has multiple children, a ghost variable based
    /// branching is needed.
    Multiple {
        children: Vec<ReborrowingNode>,
    }
}

pub struct ReborrowingNode {
    kind: ReborrowingKind,
    branching: ReborrowingBranching,
}

impl fmt::Debug for ReborrowingNode {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            ReborrowingKind::Assignment { ref loan } => {
                write!(f, "{:?}", loan)?;
            }
            ReborrowingKind::Call { ref loan, ref variable, ref region } => {
                write!(f, "Call({:?}, {:?}:{:?})", loan, variable, region)?;
            }
            ReborrowingKind::Loop => {
                unimplemented!();
            }
        }
        match self.branching {
            ReborrowingBranching::Leaf => {
                write!(f, "▪")?;
            }
            ReborrowingBranching::Single { box ref child }  => {
                write!(f, "→")?;
                child.fmt(f)?;
            }
            ReborrowingBranching::Multiple { ref children }  => {
                write!(f, "→ ⟦")?;
                for child in children.iter() {
                    child.fmt(f)?;
                    write!(f, ",")?;
                }
                write!(f, "⟧")?;
            }
        }
        Ok(())
    }
}

pub struct ReborrowingTree {
    root: ReborrowingNode,
}

impl fmt::Debug for ReborrowingTree {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.root.fmt(f)
    }
}

pub struct ReborrowingForest {
    trees: Vec<ReborrowingTree>,
}

impl ToString for ReborrowingForest {

    fn to_string(&self) -> String {
        let trees: Vec<_> = self.trees.iter().map(|tree| format!("{:?}", tree)).collect();
        trees.join(";")
    }
}

pub struct PoloniusInfo<'a, 'tcx: 'a> {
    mir: &'a mir::Mir<'tcx>,
    pub(crate) borrowck_in_facts: facts::AllInputFacts,
    pub(crate) borrowck_out_facts: facts::AllOutputFacts,
    pub(crate) interner: facts::Interner,
    /// Position at which a specific loan was created.
    pub(crate) loan_position: HashMap<facts::Loan, mir::Location>,
    pub(crate) call_magic_wands: HashMap<facts::Loan, mir::Local>,
    pub(crate) variable_regions: HashMap<mir::Local, facts::Region>,
    pub(crate) additional_facts: AdditionalFacts,
    /// Loop head → Vector of magic wands in that loop.
    pub(crate) loop_magic_wands: HashMap<mir::BasicBlock, Vec<LoopMagicWand>>,
}

impl<'a, 'tcx: 'a> PoloniusInfo<'a, 'tcx> {
    pub fn new(tcx: ty::TyCtxt, def_id: DefId, mir: &'a mir::Mir<'tcx>) -> Self {
        // Read Polonius facts.
        let def_path = tcx.hir.def_path(def_id);
        let dir_path = PathBuf::from("nll-facts").join(def_path.to_filename_friendly_no_crate());
        debug!("Reading facts from: {:?}", dir_path);
        let mut facts_loader = facts::FactLoader::new();
        facts_loader.load_all_facts(&dir_path);

        // Read relations between region IDs and local variables.
        let renumber_path = PathBuf::from(format!(
            "log/mir/rustc.{}.-------.renumber.0.mir",
            def_path.to_filename_friendly_no_crate()));
        debug!("Renumber path: {:?}", renumber_path);
        let variable_regions = regions::load_variable_regions(&renumber_path).unwrap();

        //let mir = tcx.mir_validated(def_id).borrow();

        let mut call_magic_wands = HashMap::new();

        let mut all_facts = facts_loader.facts;
        {
            // TODO: Refactor.
            // The code that adds a creation of a new borrow for each
            // move of a borrow.

            // Find the last loan index.
            let mut last_loan_id = 0;
            for (_, loan, _) in all_facts.borrow_region.iter() {
                if loan.index() > last_loan_id {
                    last_loan_id = loan.index();
                }
            }
            last_loan_id += 1;

            // Create a map from points to (region1, region2) vectors.
            let universal_region = &all_facts.universal_region;
            let mut outlives_at_point = HashMap::new();
            for (region1, region2, point) in all_facts.outlives.iter() {
                if !universal_region.contains(region1) && !universal_region.contains(region2) {
                    let outlives = outlives_at_point.entry(point).or_insert(vec![]);
                    outlives.push((region1, region2));
                }
            }

            // Create new borrow_region facts for points where is only one outlives
            // fact and there is not a borrow_region fact already.
            let borrow_region = &mut all_facts.borrow_region;
            for (point, mut regions) in outlives_at_point {
                if borrow_region.iter().all(|(_, _, loan_point)| loan_point != point) {
                    let location = facts_loader.interner.get_point(*point).location.clone();
                    if regions.len() > 1 {
                        let call_destination = get_call_destination(&mir, location);
                        if let Some(place) = call_destination {
                            debug!("Adding for call destination:");
                            for &(region1, region2) in regions.iter() {
                                debug!("{:?} {:?} {:?}", location, region1, region2);
                            }
                            match place {
                                mir::Place::Local(local) => {
                                    if let Some(var_region) = variable_regions.get(&local) {
                                        debug!("var_region = {:?} loan = {}", var_region, last_loan_id);
                                        let loan = facts::Loan::from(last_loan_id);
                                        borrow_region.push(
                                            (*var_region,
                                             loan,
                                             *point));
                                        last_loan_id += 1;
                                        call_magic_wands.insert(loan, local);
                                    }
                                }
                                x => unimplemented!("{:?}", x)
                            }
                        }
                    } else if is_assignment(&mir, location) {
                        let (_region1, region2) = regions.pop().unwrap();
                        borrow_region.push((*region2, facts::Loan::from(last_loan_id), *point));
                        debug!("Adding generic: {:?} {:?} {:?} {}", _region1, region2, location, last_loan_id);
                        last_loan_id += 1;
                    }
                }
            }
        }

        let output = Output::compute(&all_facts, Algorithm::Naive, true);

        let interner = facts_loader.interner;
        let loan_position = all_facts.borrow_region
            .iter()
            .map(|&(_, loan, point_index)| {
                let point = interner.get_point(point_index);
                (loan, point.location)
            })
            .collect();

        let additional_facts = AdditionalFacts::new(&all_facts, &output);

        PoloniusInfo {
            mir: mir,
            borrowck_in_facts: all_facts,
            borrowck_out_facts: output,
            interner: interner,
            loan_position: loan_position,
            call_magic_wands: call_magic_wands,
            variable_regions: variable_regions,
            additional_facts: additional_facts,
            loop_magic_wands: HashMap::new(),
        }
    }

    fn get_point(&self, location: mir::Location, point_type: facts::PointType) -> facts::PointIndex {
        let point = facts::Point {
            location: location,
            typ: point_type,
        };
        self.interner.get_point_index(&point)
    }

    /// Get loans that dye at the given location.
    pub(crate) fn get_dying_loans(&self, location: mir::Location) -> Vec<facts::Loan> {
        let start_point = self.get_point(location, facts::PointType::Start);
        let mid_point = self.get_point(location, facts::PointType::Mid);

        if let Some(mid_loans) = self.borrowck_out_facts.borrow_live_at.get(&mid_point) {
            let mut mid_loans = mid_loans.clone();
            mid_loans.sort();
            let default_vec = vec![];
            let start_loans = self.borrowck_out_facts
                .borrow_live_at
                .get(&start_point)
                .unwrap_or(&default_vec);
            let mut start_loans = start_loans.clone();
            start_loans.sort();
            debug!("start_loans = {:?}", start_loans);
            debug!("mid_loans = {:?}", mid_loans);
            // Loans are created in mid point, so mid_point may contain more loans than the start
            // point.
            assert!(start_loans.iter().all(|loan| mid_loans.contains(loan)));

            let successors = self.get_successors(location);

            // Filter loans that are not missing in all successors.
            mid_loans
                .into_iter()
                .filter(|loan| {
                    !successors
                        .iter()
                        .any(|successor_location| {
                            let point = self.get_point(*successor_location, facts::PointType::Start);
                            self.borrowck_out_facts
                                .borrow_live_at
                                .get(&point)
                                .map_or(false, |successor_loans| {
                                    successor_loans.contains(loan)
                                })
                        })
                })
                .collect()
        } else {
            assert!(self.borrowck_out_facts.borrow_live_at.get(&start_point).is_none());
            vec![]
        }
    }

    /// Find minimal elements that are the tree roots.
    fn find_loan_roots(&self, loans: &[facts::Loan]) -> Vec<facts::Loan> {
        let mut roots = Vec::new();
        for &loan in loans.iter() {
            let is_smallest = !loans
                .iter()
                .any(|&other_loan| {
                    self.additional_facts.reborrows.contains(&(other_loan, loan))
                });
            debug!("loan={:?} is_smallest={}", loan, is_smallest);
            if is_smallest {
                roots.push(loan);
            }
        }
        roots
    }

    pub fn construct_reborrowing_forest(&self, loans: &[facts::Loan]) -> ReborrowingForest {
        let roots = self.find_loan_roots(loans);

        // Reconstruct the tree from each root.
        let mut trees = Vec::new();
        for &root in roots.iter() {
            let tree = ReborrowingTree {
                root: self.construct_reborrowing_tree(&loans, root),
            };
            trees.push(tree);
        }

        let mut forest = ReborrowingForest {
            trees: trees,
        };
        forest
    }

    fn construct_reborrowing_tree(&self, loans: &[facts::Loan],
                                  node: facts::Loan) -> ReborrowingNode {

        let kind = if let Some(local) = self.call_magic_wands.get(&node) {
            let region = self.variable_regions[&local];
            ReborrowingKind::Call {
                loan: node,
                variable: *local,
                region: region,
            }
        } else {
            ReborrowingKind::Assignment {
                loan: node,
            }
        };
        let mut children = Vec::new();
        for &loan in loans.iter() {
            if self.additional_facts.reborrows_direct.contains(&(node, loan)) {
                children.push(loan);
            }
        }
        let branching = if children.len() == 1 {
            let child = children.pop().unwrap();
            ReborrowingBranching::Single {
                child: box self.construct_reborrowing_tree(loans, child),
            }
        } else if children.len() > 1 {
            ReborrowingBranching::Multiple {
                children: children.iter().map(|&child| {
                    self.construct_reborrowing_tree(loans, child)
                }).collect(),
            }
        } else {
            ReborrowingBranching::Leaf
        };
        ReborrowingNode {
            kind: kind,
            branching: branching,
        }
    }

    pub fn add_loop_magic_wand(&mut self, loop_head: mir::BasicBlock,
                               loop_info: &loops::ProcedureLoops,
                               local: mir::Local) {
        let region = self.variable_regions[&local];
        let location = mir::Location {
            block: loop_head,
            statement_index: 0,
        };
        let point = self.get_point(location, facts::PointType::Start);
        let restricts_map = &self.borrowck_out_facts.restricts;
        let restricts_at_point = restricts_map.get(&point);
        let restricts = restricts_at_point.as_ref().expect("BUG: No restricts");
        let loans = restricts.get(&region).expect("BUG: no loans");
        let loans: Vec<_> = loans
            .iter()
            .filter(|loan| {
                // Drop all loans that are from inside of the loop.
                let loan_block = self.loan_position[loan].block;
                !loop_info.is_block_in_loop(loop_head, loan_block)
            })
            .cloned()
            .collect();
        let roots = self.find_loan_roots(&loans);
        let magic_wand = LoopMagicWand {
            loop_id: loop_head,
            variable: local,
            region: region,
            loans: roots,
        };
        let mut entry = self.loop_magic_wands.entry(loop_head).or_insert(Vec::new());
        entry.push(magic_wand);
    }

    pub fn get_expiring_borrows(&self, location: mir::Location) -> Vec<ExpiringBorrow<'tcx>> {
        let mut expiring_borrows = vec![];
        for loan in self.get_dying_loans(location).iter() {
            let loan_location = self.loan_position[loan];
            let mir_block = &self.mir[loan_location.block];
            debug_assert!(loan_location.statement_index < mir_block.statements.len());
            let mir_stmt = &mir_block.statements[loan_location.statement_index];
            match mir_stmt.kind {
                mir::StatementKind::Assign(ref lhs_place, ref rvalue) => {
                    expiring_borrows.push(
                        ExpiringBorrow {
                            expiring: lhs_place.clone(),
                            restored: rvalue.clone(),
                            location: loan_location
                        }
                    )
                }

                ref x => unreachable!("Borrow starts at statement {:?}", x),
            }
        }
        expiring_borrows
    }

    fn get_successors(&self, location: mir::Location) -> Vec<mir::Location> {
        let statements_len = self.mir[location.block].statements.len();
        if location.statement_index < statements_len {
            vec![mir::Location {
                statement_index: location.statement_index + 1,
                .. location
            }]
        } else {
            let mut successors = Vec::new();
            for successor in self.mir[location.block].terminator.as_ref().unwrap().successors() {
                successors.push(mir::Location {
                    block: *successor,
                    statement_index: 0,
                });
            }
            successors
        }
    }

    /*
    /// `package` – should it also try to compute the package statements?
    pub fn write_magic_wands(&mut self, package: bool,
                         location: mir::Location) -> Result<(), io::Error> {
        // TODO: Refactor out this code that computes magic wands.
        let blocker = mir::RETURN_PLACE;
        //TODO: Check if it really is always start and not the mid point.
        let start_point = self.get_point(location, facts::PointType::Start);

        if let Some(region) = self.variable_regions.get(&blocker) {
            write_graph!(self, "<tr>");
            write_graph!(self, "<td colspan=\"2\">Magic wand</td>");
            let subset_map = &self.borrowck_out_facts.subset;
            if let Some(ref subset) = subset_map.get(&start_point).as_ref() {
                let mut blocked_variables = Vec::new();
                if let Some(blocked_regions) = subset.get(&region) {
                    for blocked_region in blocked_regions.iter() {
                        if blocked_region == region {
                            continue;
                        }
                        if let Some(local) = self.find_variable(*blocked_region) {
                            blocked_variables.push(format!("{:?}:{:?}", local, blocked_region));
                        }
                    }
                    write_graph!(self, "<td colspan=\"7\">{:?}:{:?} --* {}</td>",
                                 blocker, region, to_sorted_string!(blocked_variables));
                } else {
                    write_graph!(self, "<td colspan=\"7\">BUG: no blocked region</td>");
                }
            } else {
                write_graph!(self, "<td colspan=\"7\">BUG: no subsets</td>");
            }
            write_graph!(self, "</tr>");
            if package {
                let restricts_map = &self.borrowck_out_facts.restricts;
                write_graph!(self, "<tr>");
                write_graph!(self, "<td colspan=\"2\">Package</td>");
                if let Some(ref restricts) = restricts_map.get(&start_point).as_ref() {
                    if let Some(loans) = restricts.get(&region) {
                        let loans: Vec<_> = loans.iter().cloned().collect();
                        write_graph!(self, "<td colspan=\"7\">{}", to_sorted_string!(loans));
                        self.write_reborrowing_trees(&loans)?;
                        write_graph!(self, "</td>");
                    } else {
                        write_graph!(self, "<td colspan=\"7\">BUG: no loans</td>");
                    }
                } else {
                    write_graph!(self, "<td colspan=\"7\">BUG: no restricts</td>");
                }
                write_graph!(self, "</tr>");
            }
        }
        Ok(())
    }

    fn write_reborrowing_trees(&self, loans: &[facts::Loan]) -> Result<(), io::Error> {
        // Find minimal elements that are the tree roots.
        let mut roots = Vec::new();
        for &loan in loans.iter() {
            let is_smallest = !loans
                .iter()
                .any(|&other_loan| {
                    self.additional_facts.reborrows.contains(&(other_loan, loan))
                });
            if is_smallest {
                roots.push(loan);
            }
        }
        // Reconstruct the tree from each root.
        for &root in roots.iter() {
            write_graph!(self, "<br />");
            self.write_reborrowing_tree(loans, root)?;
        }
        Ok(())
    }

    fn write_reborrowing_tree(&self, loans: &[facts::Loan],
                              node: facts::Loan) -> Result<(), io::Error> {
        if let Some(local) = self.call_magic_wands.get(&node) {
            let var_region = self.variable_regions[&local];
            write_graph!(self, "apply({:?}, {:?}:{:?})", node, local, var_region);
        } else {
            write_graph!(self, "{:?}", node);
        }
        let mut children = Vec::new();
        for &loan in loans.iter() {
            if self.additional_facts.reborrows_direct.contains(&(node, loan)) {
                children.push(loan);
            }
        }
        if children.len() == 1 {
            write_graph!(self, "{}", to_html_display!("->"));
            let child = children.pop().unwrap();
            self.write_reborrowing_tree(loans, child);
        } else if children.len() > 1 {
            write_graph!(self, "{}", to_html_display!("-> ("));
            for child in children {
                self.write_reborrowing_tree(loans, child);
                write_graph!(self, ",");
            }
            write_graph!(self, ")");
        }
        Ok(())
    }
    */
}

/// Extract the call terminator at the location. Otherwise return None.
fn is_assignment<'tcx>(mir: &mir::Mir<'tcx>,
                 location: mir::Location) -> bool {
    let mir::BasicBlockData { ref statements, .. } = mir[location.block];
    if statements.len() == location.statement_index {
        return false;
    }
    match statements[location.statement_index].kind {
        mir::StatementKind::Assign { .. } => true,
        _ => false,
    }
}

/// Extract the call terminator at the location. Otherwise return None.
fn get_call_destination<'tcx>(mir: &mir::Mir<'tcx>,
                              location: mir::Location) -> Option<mir::Place<'tcx>> {
    let mir::BasicBlockData { ref statements, ref terminator, .. } = mir[location.block];
    if statements.len() != location.statement_index {
        return None;
    }
    match terminator.as_ref().unwrap().kind {
        mir::TerminatorKind::Call { ref destination, .. } => {
            if let Some((ref place, _)) = destination {
                Some(place.clone())
            } else {
                None
            }
        }
        ref x => {
            panic!("Expected call, got {:?} at {:?}", x, location);
        }
    }
}


/// Additional facts derived from the borrow checker facts.
pub struct AdditionalFacts {
    /// A list of loans sorted by id.
    pub loans: Vec<facts::Loan>,
    /// The ``reborrows`` facts are needed for removing “fake” loans: at
    /// a specific program point there are often more than one loan active,
    /// but we are interested in only one of them, which is the original one.
    /// Therefore, we find all loans that are reborrows of the original loan
    /// and remove them. Reborrowing is defined as follows:
    ///
    /// ```datalog
    /// reborrows(Loan, Loan);
    /// reborrows(L1, L2) :-
    ///     borrow_region(R, L1, P),
    ///     restricts(R, P, L2).
    /// reborrows(L1, L3) :-
    ///     reborrows(L1, L2),
    ///     reborrows(L2, L3).
    /// ```
    pub reborrows: Vec<(facts::Loan, facts::Loan)>,
    /// Non-transitive `reborrows`.
    pub reborrows_direct: Vec<(facts::Loan, facts::Loan)>,
    /// The ``zombie_requires`` facts are ``requires`` facts for the loans
    /// that were killed.
    ///
    /// ```datalog
    /// zombie_requires(Region, Loan, Point);
    /// zombie_requires(R, L, Q) :-
    ///     requires(R, L, P),
    ///     killed(L, P),
    ///     cfg_edge(P, Q),
    ///     region_live_at(R, Q).
    /// zombie_requires(R2, L, P) :-
    ///     zombie_requires(R1, L, P),
    ///     subset(R1, R2, P).
    /// zombie_requires(R, L, Q) :-
    ///     zombie_requires(R, L, P),
    ///     cfg_edge(P, Q),
    ///     region_live_at(R, Q).
    /// ```
    pub zombie_requires: HashMap<facts::PointIndex, HashMap<facts::Region, HashSet<facts::Loan>>>,
    /// The ``zombie_borrow_live_at`` facts are ``borrow_live_at`` facts
    /// for the loans that were killed.
    ///
    /// ```datalog
    /// zombie_borrow_live_at(L, P) :-
    ///     zombie_requires(R, L, P),
    ///     region_live_at(R, P).
    /// ```
    pub zombie_borrow_live_at: HashMap<facts::PointIndex, Vec<facts::Loan>>,
}


impl AdditionalFacts {

    /// Derive ``zombie_requires``.
    fn derive_zombie_requires(all_facts: &facts::AllInputFacts,
                              output: &facts::AllOutputFacts
                              ) -> (
                                  HashMap<facts::PointIndex, HashMap<facts::Region, HashSet<facts::Loan>>>,
                                  HashMap<facts::PointIndex, Vec<facts::Loan>>) {

        use datafrog::{Iteration, Relation};
        use self::facts::{PointIndex as Point, Loan, Region};

        let mut iteration = Iteration::new();

        // Variables that are outputs of our computation.
        let zombie_requires = iteration.variable::<(Region, Loan, Point)>("zombie_requires");
        let zombie_borrow_live_at = iteration.variable::<(Loan, Point)>("zombie_borrow_live_at");

        // Variables for initial data.
        let requires_lp = iteration.variable::<((Loan, Point), Region)>("requires_lp");
        let killed = iteration.variable::<((Loan, Point), ())>("killed");
        let cfg_edge_p = iteration.variable::<(Point, Point)>("cfg_edge_p");
        let region_live_at = iteration.variable::<((Region, Point), ())>("region_live_at");
        let subset_r1p = iteration.variable::<((Region, Point), Region)>("subset_r1p");

        // Temporaries as we perform a multi-way join.
        let zombie_requires_rp = iteration.variable_indistinct("zombie_requires_rp");
        let zombie_requires_p = iteration.variable_indistinct("zombie_requires_p");
        let zombie_requires_1 = iteration.variable_indistinct("zombie_requires_1");
        let zombie_requires_2 = iteration.variable_indistinct("zombie_requires_2");
        let zombie_requires_3 = iteration.variable_indistinct("zombie_requires_3");

        // Load initial facts.
        requires_lp.insert(Relation::from(
            output.restricts.iter().flat_map(
                |(&point, region_map)|
                region_map.iter().flat_map(
                    move |(&region, loans)|
                    loans.iter().map(move |&loan| ((loan, point), region))
                )
            )
        ));
        killed.insert(Relation::from(
            all_facts.killed.iter().map(
                |&(loan, point)|
                ((loan, point), ())
            )
        ));
        cfg_edge_p.insert(all_facts.cfg_edge.clone().into());
        region_live_at.insert(Relation::from(
            all_facts.region_live_at.iter().map(|&(r, p)| ((r, p), ())),
        ));
        subset_r1p.insert(Relation::from(
            output.subset.iter().flat_map(
                |(&point, subset_map)|
                subset_map.iter().flat_map(
                    move |(&region1, regions)|
                    regions.iter().map(move |&region2| ((region1, point), region2))
                )
            )
        ));

        while iteration.changed() {

            zombie_requires_rp.from_map(&zombie_requires, |&(r, l, p)| ((r, p), l));
            zombie_requires_p.from_map(&zombie_requires, |&(r, l, p)| (p, (l, r)));

            // zombie_requires(R, L, Q) :-
            //     requires(R, L, P),
            //     killed(L, P),
            //     cfg_edge(P, Q),
            //     region_live_at(R, Q).
            zombie_requires_1.from_join(&requires_lp, &killed, |&(l, p), &r, _| (p, (l, r)));
            zombie_requires_2.from_join(&zombie_requires_1, &cfg_edge_p, |&_p, &(l, r), &q| ((r, q), l));
            zombie_requires.from_join(&zombie_requires_2, &region_live_at, |&(r, q), &l, &()| (r, l, q));

            // zombie_requires(R2, L, P) :-
            //     zombie_requires(R1, L, P),
            //     subset(R1, R2, P).
            zombie_requires.from_join(&zombie_requires_rp, &subset_r1p, |&(_r1, p), &b, &r2| (r2, b, p));

            // zombie_requires(R, L, Q) :-
            //     zombie_requires(R, L, P),
            //     cfg_edge(P, Q),
            //     region_live_at(R, Q).
            zombie_requires_3.from_join(&zombie_requires_p, &cfg_edge_p, |&_p, &(l, r), &q| ((r, q), l));
            zombie_requires.from_join(&zombie_requires_3, &region_live_at, |&(r, q), &l, &()| (r, l, q));

            // zombie_borrow_live_at(L, P) :-
            //     zombie_requires(R, L, P),
            //     region_live_at(R, P).
            zombie_borrow_live_at.from_join(&zombie_requires_rp, &region_live_at, |&(_r, p), &l, &()| (l, p));
        }

        let zombie_requires = zombie_requires.complete();
        let mut zombie_requires_map = HashMap::new();
        for (region, loan, point) in &zombie_requires.elements {
            zombie_requires_map
                .entry(*point)
                .or_insert(HashMap::new())
                .entry(*region)
                .or_insert(HashSet::new())
                .insert(*loan);
        }

        let zombie_borrow_live_at = zombie_borrow_live_at.complete();
        let mut zombie_borrow_live_at_map = HashMap::new();
        for (loan, point) in &zombie_borrow_live_at.elements {
            zombie_borrow_live_at_map
                .entry(*point)
                .or_insert(Vec::new())
                .push(*loan);
        }

        (zombie_requires_map, zombie_borrow_live_at_map)
    }

    /// Derive additional facts from the borrow checker facts.
    pub fn new(all_facts: &facts::AllInputFacts,
               output: &facts::AllOutputFacts) -> AdditionalFacts {

        use datafrog::{Iteration, Relation};
        use self::facts::{PointIndex as Point, Loan, Region};

        let mut iteration = Iteration::new();

        // Variables that are outputs of our computation.
        let reborrows = iteration.variable::<(Loan, Loan)>("reborrows");

        // Variables for initial data.
        let restricts = iteration.variable::<((Point, Region), Loan)>("restricts");
        let borrow_region = iteration.variable::<((Point, Region), Loan)>("borrow_region");

        // Load initial data.
        restricts.insert(Relation::from(
            output.restricts.iter().flat_map(
                |(&point, region_map)|
                region_map.iter().flat_map(
                    move |(&region, loans)|
                    loans.iter().map(move |&loan| ((point, region), loan))
                )
            )
        ));
        borrow_region.insert(Relation::from(
            all_facts.borrow_region.iter().map(|&(r, l, p)| ((p, r), l))
        ));

        // Temporaries for performing join.
        let reborrows_1 = iteration.variable_indistinct("reborrows_1");
        let reborrows_2 = iteration.variable_indistinct("reborrows_2");

        while iteration.changed() {

            // reborrows(L1, L2) :-
            //   borrow_region(R, L1, P),
            //   restricts(R, P, L2).
            reborrows.from_join(&borrow_region, &restricts, |_, &l1, &l2| (l1, l2));

            // Compute transitive closure of reborrows:
            // reborrows(L1, L3) :-
            //   reborrows(L1, L2),
            //   reborrows(L2, L3).
            reborrows_1.from_map(&reborrows, |&(l1, l2)| (l2, l1));
            reborrows_2.from_map(&reborrows, |&(l2, l3)| (l2, l3));
            reborrows.from_join(&reborrows_1, &reborrows_2, |_, &l1, &l3| (l1, l3));
        }

        // Remove reflexive edges.
        let reborrows: Vec<_> = reborrows
            .complete()
            .iter()
            .filter(|(l1, l2)| l1 != l2)
            .cloned()
            .collect();
        // A non-transitive version of reborrows.
        let mut reborrows_direct = Vec::new();
        for &(l1, l2) in reborrows.iter() {
            let is_l2_minimal = !reborrows
                .iter()
                .any(|&(l3, l4)| {
                    l4 == l2 && reborrows.contains(&(l1, l3))
                });
            if is_l2_minimal {
                reborrows_direct.push((l1, l2));
            }
        }
        // Compute the sorted list of all loans.
        let mut loans: Vec<_> = all_facts
            .borrow_region
            .iter()
            .map(|&(_, l, _)| l)
            .collect();
        loans.sort();

        let (zombie_requires, zombie_borrow_live_at) =
            Self::derive_zombie_requires(all_facts, output);
        AdditionalFacts {
            loans: loans,
            reborrows: reborrows,
            reborrows_direct: reborrows_direct,
            zombie_requires: zombie_requires,
            zombie_borrow_live_at: zombie_borrow_live_at,
        }
    }

}
