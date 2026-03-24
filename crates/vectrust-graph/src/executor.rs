use std::collections::HashMap;
use uuid::Uuid;
use vectrust_core::{GraphNode, GraphQueryResult, GraphValue, Result, ResultRow, VectraError};
use vectrust_cypher::{
    ArithmeticOp, BooleanOp, CallClause, Clause, ComparisonOp, CreateClause, DeleteClause,
    Direction, Expression, LimitClause, Literal, MatchClause, OrderByClause, Pattern,
    PatternElement, RemoveClause, ReturnClause, SetClause, SetItem, SkipClause, Statement,
    StringMatchOp, WhereClause,
};

use crate::storage::GraphStorage;

/// Executes Cypher AST against a GraphStorage instance.
pub struct GraphExecutor<'a> {
    storage: &'a GraphStorage,
    params: HashMap<String, GraphValue>,
}

impl<'a> GraphExecutor<'a> {
    pub fn new(storage: &'a GraphStorage, params: HashMap<String, GraphValue>) -> Self {
        Self { storage, params }
    }

    /// Execute a parsed Cypher statement.
    pub fn execute(&self, stmt: &Statement) -> Result<GraphQueryResult> {
        // Working set: rows of variable bindings
        let mut rows: Vec<ResultRow> = Vec::new();
        let mut has_rows = false;
        let mut columns: Vec<String> = Vec::new();
        let mut has_return = false;

        for clause in &stmt.clauses {
            match clause {
                Clause::Create(c) => {
                    rows = self.execute_create(c, &rows, has_rows)?;
                    has_rows = true;
                }
                Clause::Match(m) => {
                    rows = self.execute_match(m, &rows, has_rows)?;
                    has_rows = true;
                }
                Clause::Where(w) => {
                    rows = self.execute_where(w, &rows)?;
                }
                Clause::Return(r) => {
                    let result = self.execute_return(r, &rows)?;
                    columns = result.columns;
                    rows = result.rows;
                    has_return = true;
                }
                Clause::Set(s) => {
                    self.execute_set(s, &rows)?;
                }
                Clause::Delete(d) => {
                    self.execute_delete(d, &rows)?;
                }
                Clause::OrderBy(o) => {
                    rows = self.execute_order_by(o, rows)?;
                }
                Clause::Limit(l) => {
                    rows = self.execute_limit(l, rows)?;
                }
                Clause::Skip(s) => {
                    rows = self.execute_skip(s, rows)?;
                }
                Clause::With(w) => {
                    let return_clause = ReturnClause {
                        distinct: w.distinct,
                        items: w.items.clone(),
                    };
                    let result = self.execute_return(&return_clause, &rows)?;
                    rows = result.rows;
                    has_rows = true;
                }
                Clause::Remove(r) => {
                    self.execute_remove(r, &rows)?;
                }
                Clause::Call(c) => {
                    rows = self.execute_call(c)?;
                    has_rows = true;
                }
            }
        }

        if has_return {
            Ok(GraphQueryResult { columns, rows })
        } else {
            Ok(GraphQueryResult {
                columns: Vec::new(),
                rows,
            })
        }
    }

    // ─── CREATE ──────────────────────────────────────────────────

    fn execute_create(
        &self,
        clause: &CreateClause,
        existing_rows: &[ResultRow],
        has_existing: bool,
    ) -> Result<Vec<ResultRow>> {
        if has_existing && !existing_rows.is_empty() {
            // CREATE with existing bindings: execute once per row
            let mut result_rows = Vec::new();
            for row in existing_rows {
                let mut new_row = row.clone();
                self.create_pattern(&clause.patterns, &mut new_row)?;
                result_rows.push(new_row);
            }
            Ok(result_rows)
        } else {
            // Standalone CREATE
            let mut row = ResultRow::new();
            self.create_pattern(&clause.patterns, &mut row)?;
            Ok(vec![row])
        }
    }

    fn create_pattern(&self, patterns: &[Pattern], bindings: &mut ResultRow) -> Result<()> {
        for pattern in patterns {
            let mut last_node_id: Option<Uuid> = None;

            for element in &pattern.elements {
                match element {
                    PatternElement::Node(np) => {
                        // Check if variable already bound
                        let node_id = if let Some(ref var) = np.variable {
                            if let Some(existing) = bindings.get(var) {
                                if let Some(node) = existing.as_node() {
                                    node.id
                                } else {
                                    return Err(VectraError::Graph {
                                        message: format!("Variable '{}' is not a node", var),
                                    });
                                }
                            } else {
                                // Create new node
                                let props =
                                    self.eval_map_properties(np.properties.as_ref(), bindings)?;
                                let id = self.storage.create_node(&np.labels, props)?;
                                let node = self.storage.get_node(id)?.ok_or_else(|| {
                                    VectraError::Graph {
                                        message: "Failed to read created node".into(),
                                    }
                                })?;
                                bindings.insert(var.clone(), GraphValue::Node(node));
                                id
                            }
                        } else {
                            // Anonymous node
                            let props =
                                self.eval_map_properties(np.properties.as_ref(), bindings)?;
                            self.storage.create_node(&np.labels, props)?
                        };
                        last_node_id = Some(node_id);
                    }
                    PatternElement::Relationship(rp) => {
                        let source_id = last_node_id.ok_or_else(|| VectraError::Graph {
                            message: "Relationship without source node".into(),
                        })?;

                        // Peek ahead to get target node
                        // (The target node will be created in the next iteration)
                        // For now, store source for the edge.
                        // We'll create the edge when we have both endpoints.
                        // Actually, we need to defer edge creation. Let's use a different approach:
                        // collect node IDs and create edges after all nodes.

                        // For simplicity in this MVP, we rely on the pattern structure:
                        // Node, Rel, Node, Rel, Node...
                        // The edge will be created when the next node is processed.
                        // Store pending edge info in bindings with a special key.
                        let rel_type = rp.rel_types.first().cloned().unwrap_or_default();
                        let props = self.eval_map_properties(rp.properties.as_ref(), bindings)?;
                        let direction = rp.direction.clone();
                        let var = rp.variable.clone();

                        // Store pending edge
                        bindings.insert(
                            "__pending_edge".to_string(),
                            GraphValue::Map(HashMap::from([
                                (
                                    "source".to_string(),
                                    GraphValue::String(source_id.to_string()),
                                ),
                                ("rel_type".to_string(), GraphValue::String(rel_type)),
                                (
                                    "direction".to_string(),
                                    GraphValue::String(format!("{:?}", direction)),
                                ),
                                (
                                    "variable".to_string(),
                                    var.map(GraphValue::String).unwrap_or(GraphValue::Null),
                                ),
                                (
                                    "properties".to_string(),
                                    GraphValue::Map(props.into_iter().collect()),
                                ),
                            ])),
                        );
                        last_node_id = Some(source_id);
                    }
                }

                // Check for pending edge after creating a node
                if matches!(element, PatternElement::Node(_)) {
                    if let Some(GraphValue::Map(edge_info)) = bindings.remove("__pending_edge") {
                        let source_str = edge_info
                            .get("source")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| VectraError::Graph {
                                message: "Invalid pending edge".into(),
                            })?;
                        let source =
                            Uuid::parse_str(source_str).map_err(|e| VectraError::Graph {
                                message: e.to_string(),
                            })?;
                        let rel_type = edge_info
                            .get("rel_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let target = last_node_id.ok_or_else(|| VectraError::Graph {
                            message: "Edge without target node".into(),
                        })?;
                        let direction_str = edge_info
                            .get("direction")
                            .and_then(|v| v.as_str())
                            .unwrap_or("OutRight");

                        let props = if let Some(GraphValue::Map(p)) = edge_info.get("properties") {
                            p.clone()
                        } else {
                            HashMap::new()
                        };

                        let (actual_source, actual_target) = if direction_str.contains("InLeft") {
                            (target, source)
                        } else {
                            (source, target)
                        };

                        let edge_id = self.storage.create_edge(
                            actual_source,
                            actual_target,
                            rel_type,
                            props,
                        )?;

                        // Bind edge variable if present
                        if let Some(GraphValue::String(var)) = edge_info.get("variable") {
                            if let Some(edge) = self.storage.get_edge(edge_id)? {
                                bindings.insert(var.clone(), GraphValue::Edge(edge));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // ─── MATCH ───────────────────────────────────────────────────

    fn execute_match(
        &self,
        clause: &MatchClause,
        existing_rows: &[ResultRow],
        has_existing: bool,
    ) -> Result<Vec<ResultRow>> {
        let mut all_rows = Vec::new();

        for pattern in &clause.patterns {
            let pattern_rows = if has_existing && !existing_rows.is_empty() {
                let mut result = Vec::new();
                for row in existing_rows {
                    let matched = self.match_pattern(pattern, row)?;
                    result.extend(matched);
                }
                result
            } else {
                self.match_pattern(pattern, &ResultRow::new())?
            };
            all_rows = if all_rows.is_empty() {
                pattern_rows
            } else {
                // Cross-product for multiple patterns (comma-separated)
                let mut cross = Vec::new();
                for existing in &all_rows {
                    for new in &pattern_rows {
                        let mut merged = existing.clone();
                        merged.extend(new.clone());
                        cross.push(merged);
                    }
                }
                cross
            };
        }

        Ok(all_rows)
    }

    fn match_pattern(
        &self,
        pattern: &Pattern,
        initial_bindings: &ResultRow,
    ) -> Result<Vec<ResultRow>> {
        // Start by matching the first node
        let first_element = pattern.elements.first().ok_or_else(|| VectraError::Graph {
            message: "Empty pattern".into(),
        })?;

        let PatternElement::Node(first_np) = first_element else {
            return Err(VectraError::Graph {
                message: "Pattern must start with a node".into(),
            });
        };

        // Get candidate nodes for the first node
        let candidates = self.find_node_candidates(first_np, initial_bindings)?;

        let mut current_rows: Vec<ResultRow> = candidates
            .into_iter()
            .map(|node| {
                let mut row = initial_bindings.clone();
                if let Some(ref var) = first_np.variable {
                    row.insert(var.clone(), GraphValue::Node(node));
                }
                row
            })
            .collect();

        // Process relationship-node pairs
        let mut i = 1;
        while i + 1 < pattern.elements.len() {
            let PatternElement::Relationship(ref rp) = pattern.elements[i] else {
                return Err(VectraError::Graph {
                    message: "Expected relationship at pattern position".into(),
                });
            };
            let PatternElement::Node(ref next_np) = pattern.elements[i + 1] else {
                return Err(VectraError::Graph {
                    message: "Expected node after relationship".into(),
                });
            };

            let mut new_rows = Vec::new();
            for row in &current_rows {
                let expanded = self.expand_and_bind(row, rp, next_np, &pattern.elements[i - 1])?;
                new_rows.extend(expanded);
            }
            current_rows = new_rows;
            i += 2;
        }

        Ok(current_rows)
    }

    fn find_node_candidates(
        &self,
        np: &vectrust_cypher::NodePattern,
        bindings: &ResultRow,
    ) -> Result<Vec<GraphNode>> {
        // If variable is already bound, use that
        if let Some(ref var) = np.variable {
            if let Some(existing) = bindings.get(var) {
                if let Some(node) = existing.as_node() {
                    // Verify labels match
                    if np.labels.iter().all(|l| node.labels.contains(l)) {
                        return Ok(vec![node.clone()]);
                    } else {
                        return Ok(vec![]);
                    }
                }
            }
        }

        // Find nodes by label
        let node_ids = if let Some(label) = np.labels.first() {
            self.storage.nodes_by_label(label)?
        } else {
            self.storage.all_nodes()?
        };

        let mut nodes = Vec::new();
        for id in node_ids {
            if let Some(node) = self.storage.get_node(id)? {
                // Check all labels match
                if np.labels.iter().all(|l| node.labels.contains(l)) {
                    nodes.push(node);
                }
            }
        }

        Ok(nodes)
    }

    fn expand_and_bind(
        &self,
        row: &ResultRow,
        rp: &vectrust_cypher::RelationshipPattern,
        next_np: &vectrust_cypher::NodePattern,
        prev_element: &PatternElement,
    ) -> Result<Vec<ResultRow>> {
        let source_id = self.get_source_id(row, prev_element)?;

        // Variable-length paths: BFS traversal
        if let Some(length_range) = &rp.length {
            return self.expand_variable_length(row, source_id, rp, next_np, *length_range);
        }

        // Single-hop expansion
        self.expand_single_hop(row, source_id, rp, next_np)
    }

    fn get_source_id(&self, row: &ResultRow, prev_element: &PatternElement) -> Result<Uuid> {
        match prev_element {
            PatternElement::Node(np) => {
                if let Some(ref var) = np.variable {
                    row.get(var)
                        .and_then(|v| v.as_node())
                        .map(|n| n.id)
                        .ok_or_else(|| VectraError::Graph {
                            message: format!("Variable '{}' not bound to a node", var),
                        })
                } else {
                    Err(VectraError::Graph {
                        message: "Cannot expand from anonymous node in MATCH".into(),
                    })
                }
            }
            _ => Err(VectraError::Graph {
                message: "Expected node before relationship".into(),
            }),
        }
    }

    fn expand_single_hop(
        &self,
        row: &ResultRow,
        source_id: Uuid,
        rp: &vectrust_cypher::RelationshipPattern,
        next_np: &vectrust_cypher::NodePattern,
    ) -> Result<Vec<ResultRow>> {
        let rel_types: Vec<String> = rp.rel_types.clone();

        let pairs = match rp.direction {
            Direction::OutRight => self.storage.expand_out(source_id, &rel_types)?,
            Direction::InLeft => self.storage.expand_in(source_id, &rel_types)?,
            Direction::Both => self.storage.expand_both(source_id, &rel_types)?,
        };

        let mut results = Vec::new();
        for (edge, neighbor) in pairs {
            // Check target node labels
            if !next_np.labels.iter().all(|l| neighbor.labels.contains(l)) {
                continue;
            }

            // Check if target variable is already bound
            if let Some(ref var) = next_np.variable {
                if let Some(existing) = row.get(var) {
                    if let Some(existing_node) = existing.as_node() {
                        if existing_node.id != neighbor.id {
                            continue;
                        }
                    }
                }
            }

            let mut new_row = row.clone();
            if let Some(ref var) = rp.variable {
                new_row.insert(var.clone(), GraphValue::Edge(edge));
            }
            if let Some(ref var) = next_np.variable {
                new_row.insert(var.clone(), GraphValue::Node(neighbor));
            }
            results.push(new_row);
        }

        Ok(results)
    }

    /// BFS traversal for variable-length path patterns like `*1..3`.
    fn expand_variable_length(
        &self,
        row: &ResultRow,
        source_id: Uuid,
        rp: &vectrust_cypher::RelationshipPattern,
        next_np: &vectrust_cypher::NodePattern,
        (min, max): (Option<u32>, Option<u32>),
    ) -> Result<Vec<ResultRow>> {
        let min_depth = min.unwrap_or(1) as usize;
        let max_depth = max.unwrap_or(10) as usize; // Default cap to prevent infinite traversal
        let rel_types: Vec<String> = rp.rel_types.clone();

        let mut results = Vec::new();

        // BFS: frontier is (node_id, depth, visited_set)
        // Each entry tracks the path to avoid cycles
        let mut frontier: Vec<(Uuid, usize, std::collections::HashSet<Uuid>)> = Vec::new();
        let mut initial_visited = std::collections::HashSet::new();
        initial_visited.insert(source_id);
        frontier.push((source_id, 0, initial_visited));

        while let Some((current_id, depth, visited)) = frontier.pop() {
            if depth >= max_depth {
                continue;
            }

            let pairs = match rp.direction {
                Direction::OutRight => self.storage.expand_out(current_id, &rel_types)?,
                Direction::InLeft => self.storage.expand_in(current_id, &rel_types)?,
                Direction::Both => self.storage.expand_both(current_id, &rel_types)?,
            };

            for (_edge, neighbor) in pairs {
                // Cycle detection
                if visited.contains(&neighbor.id) {
                    continue;
                }

                let new_depth = depth + 1;

                // Check if this node qualifies as a result
                if new_depth >= min_depth {
                    // Check target node labels
                    if next_np.labels.iter().all(|l| neighbor.labels.contains(l)) {
                        // Check if target variable is already bound
                        let mut matches = true;
                        if let Some(ref var) = next_np.variable {
                            if let Some(existing) = row.get(var) {
                                if let Some(existing_node) = existing.as_node() {
                                    if existing_node.id != neighbor.id {
                                        matches = false;
                                    }
                                }
                            }
                        }

                        if matches {
                            let mut new_row = row.clone();
                            if let Some(ref var) = next_np.variable {
                                new_row.insert(var.clone(), GraphValue::Node(neighbor.clone()));
                            }
                            results.push(new_row);
                        }
                    }
                }

                // Continue traversal if we haven't reached max depth
                if new_depth < max_depth {
                    let mut new_visited = visited.clone();
                    new_visited.insert(neighbor.id);
                    frontier.push((neighbor.id, new_depth, new_visited));
                }
            }
        }

        Ok(results)
    }

    // ─── WHERE ───────────────────────────────────────────────────

    fn execute_where(&self, clause: &WhereClause, rows: &[ResultRow]) -> Result<Vec<ResultRow>> {
        let mut result = Vec::new();
        for row in rows {
            let val = self.eval_expr(&clause.expression, row)?;
            if is_truthy(&val) {
                result.push(row.clone());
            }
        }
        Ok(result)
    }

    // ─── RETURN ──────────────────────────────────────────────────

    fn execute_return(
        &self,
        clause: &ReturnClause,
        rows: &[ResultRow],
    ) -> Result<GraphQueryResult> {
        let columns: Vec<String> = clause
            .items
            .iter()
            .map(|item| {
                item.alias
                    .clone()
                    .unwrap_or_else(|| expr_name(&item.expression))
            })
            .collect();

        // Detect if any return items contain aggregate functions
        let has_aggregates = clause
            .items
            .iter()
            .any(|item| is_aggregate_expr(&item.expression));

        let result_rows = if has_aggregates {
            self.execute_return_with_aggregation(clause, &columns, rows)?
        } else {
            let mut result_rows = Vec::new();
            for row in rows {
                let mut result_row = ResultRow::new();
                for (item, col_name) in clause.items.iter().zip(&columns) {
                    let value = self.eval_expr(&item.expression, row)?;
                    result_row.insert(col_name.clone(), value);
                }
                result_rows.push(result_row);
            }
            result_rows
        };

        let mut result_rows = result_rows;
        if clause.distinct {
            let mut seen: Vec<Vec<(String, GraphValue)>> = Vec::new();
            result_rows.retain(|row| {
                let mut key: Vec<(String, GraphValue)> =
                    row.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                key.sort_by(|(a, _), (b, _)| a.cmp(b));
                if seen.contains(&key) {
                    false
                } else {
                    seen.push(key);
                    true
                }
            });
        }

        Ok(GraphQueryResult {
            columns,
            rows: result_rows,
        })
    }

    fn execute_return_with_aggregation(
        &self,
        clause: &ReturnClause,
        columns: &[String],
        rows: &[ResultRow],
    ) -> Result<Vec<ResultRow>> {
        // Separate grouping keys from aggregate items
        let group_indices: Vec<usize> = clause
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| !is_aggregate_expr(&item.expression))
            .map(|(i, _)| i)
            .collect();

        // If no grouping keys, aggregate all rows into one result
        if group_indices.is_empty() {
            let mut result_row = ResultRow::new();
            for (i, (item, col_name)) in clause.items.iter().zip(columns).enumerate() {
                let value = self.eval_aggregate(&item.expression, rows, i)?;
                result_row.insert(col_name.clone(), value);
            }
            return Ok(if rows.is_empty() && !clause.items.is_empty() {
                // count(*) on empty set should return 0, not empty
                vec![result_row]
            } else if rows.is_empty() {
                Vec::new()
            } else {
                vec![result_row]
            });
        }

        // Group rows by non-aggregate column values
        let mut groups: Vec<(Vec<GraphValue>, Vec<&ResultRow>)> = Vec::new();

        for row in rows {
            let group_key: Vec<GraphValue> = group_indices
                .iter()
                .map(|&i| {
                    self.eval_expr(&clause.items[i].expression, row)
                        .unwrap_or(GraphValue::Null)
                })
                .collect();

            if let Some(group) = groups.iter_mut().find(|(k, _)| *k == group_key) {
                group.1.push(row);
            } else {
                groups.push((group_key, vec![row]));
            }
        }

        let mut result_rows = Vec::new();
        for (_, group_rows) in &groups {
            let mut result_row = ResultRow::new();
            for (item, col_name) in clause.items.iter().zip(columns) {
                let value = if is_aggregate_expr(&item.expression) {
                    self.eval_aggregate_on_group(&item.expression, group_rows)?
                } else {
                    self.eval_expr(&item.expression, group_rows[0])?
                };
                result_row.insert(col_name.clone(), value);
            }
            result_rows.push(result_row);
        }

        Ok(result_rows)
    }

    fn eval_aggregate(
        &self,
        expr: &Expression,
        rows: &[ResultRow],
        _col_idx: usize,
    ) -> Result<GraphValue> {
        let row_refs: Vec<&ResultRow> = rows.iter().collect();
        self.eval_aggregate_on_group(expr, &row_refs)
    }

    fn eval_aggregate_on_group(
        &self,
        expr: &Expression,
        rows: &[&ResultRow],
    ) -> Result<GraphValue> {
        match expr {
            Expression::FunctionCall {
                name,
                args,
                distinct,
            } => {
                let func_name = name.to_lowercase();
                match func_name.as_str() {
                    "count" => {
                        if args
                            .first()
                            .map_or(false, |a| matches!(a, Expression::Star))
                        {
                            Ok(GraphValue::Integer(rows.len() as i64))
                        } else if let Some(arg) = args.first() {
                            let values: Vec<GraphValue> = rows
                                .iter()
                                .map(|r| self.eval_expr(arg, r).unwrap_or(GraphValue::Null))
                                .filter(|v| !v.is_null())
                                .collect();
                            if *distinct {
                                let mut seen: Vec<GraphValue> = Vec::new();
                                let count = values
                                    .into_iter()
                                    .filter(|v| {
                                        if seen.contains(v) {
                                            false
                                        } else {
                                            seen.push(v.clone());
                                            true
                                        }
                                    })
                                    .count();
                                Ok(GraphValue::Integer(count as i64))
                            } else {
                                Ok(GraphValue::Integer(values.len() as i64))
                            }
                        } else {
                            Ok(GraphValue::Integer(rows.len() as i64))
                        }
                    }
                    "collect" => {
                        if let Some(arg) = args.first() {
                            let values: Result<Vec<GraphValue>> =
                                rows.iter().map(|r| self.eval_expr(arg, r)).collect();
                            Ok(GraphValue::List(values?))
                        } else {
                            Ok(GraphValue::List(Vec::new()))
                        }
                    }
                    "sum" => {
                        let arg = args.first().ok_or_else(|| VectraError::Graph {
                            message: "sum() requires an argument".into(),
                        })?;
                        let mut total = 0.0f64;
                        let mut is_int = true;
                        for row in rows {
                            match self.eval_expr(arg, row)? {
                                GraphValue::Integer(n) => total += n as f64,
                                GraphValue::Float(f) => {
                                    total += f;
                                    is_int = false;
                                }
                                _ => {}
                            }
                        }
                        Ok(if is_int {
                            GraphValue::Integer(total as i64)
                        } else {
                            GraphValue::Float(total)
                        })
                    }
                    "avg" => {
                        let arg = args.first().ok_or_else(|| VectraError::Graph {
                            message: "avg() requires an argument".into(),
                        })?;
                        let mut total = 0.0f64;
                        let mut count = 0usize;
                        for row in rows {
                            match self.eval_expr(arg, row)? {
                                GraphValue::Integer(n) => {
                                    total += n as f64;
                                    count += 1;
                                }
                                GraphValue::Float(f) => {
                                    total += f;
                                    count += 1;
                                }
                                _ => {}
                            }
                        }
                        Ok(if count > 0 {
                            GraphValue::Float(total / count as f64)
                        } else {
                            GraphValue::Null
                        })
                    }
                    "min" => {
                        let arg = args.first().ok_or_else(|| VectraError::Graph {
                            message: "min() requires an argument".into(),
                        })?;
                        let mut min_val: Option<GraphValue> = None;
                        for row in rows {
                            let val = self.eval_expr(arg, row)?;
                            if val.is_null() {
                                continue;
                            }
                            min_val = Some(match min_val {
                                None => val,
                                Some(current) => {
                                    if val.partial_cmp(&current) == Some(std::cmp::Ordering::Less) {
                                        val
                                    } else {
                                        current
                                    }
                                }
                            });
                        }
                        Ok(min_val.unwrap_or(GraphValue::Null))
                    }
                    "max" => {
                        let arg = args.first().ok_or_else(|| VectraError::Graph {
                            message: "max() requires an argument".into(),
                        })?;
                        let mut max_val: Option<GraphValue> = None;
                        for row in rows {
                            let val = self.eval_expr(arg, row)?;
                            if val.is_null() {
                                continue;
                            }
                            max_val = Some(match max_val {
                                None => val,
                                Some(current) => {
                                    if val.partial_cmp(&current)
                                        == Some(std::cmp::Ordering::Greater)
                                    {
                                        val
                                    } else {
                                        current
                                    }
                                }
                            });
                        }
                        Ok(max_val.unwrap_or(GraphValue::Null))
                    }
                    _ => {
                        if let Some(row) = rows.first() {
                            self.eval_expr(expr, row)
                        } else {
                            Ok(GraphValue::Null)
                        }
                    }
                }
            }
            _ => {
                if let Some(row) = rows.first() {
                    self.eval_expr(expr, row)
                } else {
                    Ok(GraphValue::Null)
                }
            }
        }
    }

    // ─── SET ─────────────────────────────────────────────────────

    fn execute_set(&self, clause: &SetClause, rows: &[ResultRow]) -> Result<()> {
        for row in rows {
            for item in &clause.items {
                match item {
                    SetItem::Property { target, value } => {
                        if let Expression::Property { object, key } = target {
                            if let Expression::Variable(var) = object.as_ref() {
                                if let Some(node_val) = row.get(var) {
                                    if let Some(node) = node_val.as_node() {
                                        let val = self.eval_expr(value, row)?;
                                        self.storage.set_node_property(node.id, key, val)?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // ─── DELETE ──────────────────────────────────────────────────

    fn execute_delete(&self, clause: &DeleteClause, rows: &[ResultRow]) -> Result<()> {
        for row in rows {
            for expr in &clause.expressions {
                if let Expression::Variable(var) = expr {
                    if let Some(val) = row.get(var) {
                        match val {
                            GraphValue::Node(node) => {
                                self.storage.delete_node(node.id, clause.detach)?;
                            }
                            GraphValue::Edge(edge) => {
                                self.storage.delete_edge(edge.id)?;
                            }
                            _ => {
                                return Err(VectraError::Graph {
                                    message: format!(
                                        "Cannot delete variable '{}' (not a node or edge)",
                                        var
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // ─── REMOVE ──────────────────────────────────────────────────

    fn execute_remove(&self, clause: &RemoveClause, rows: &[ResultRow]) -> Result<()> {
        for row in rows {
            for expr in &clause.items {
                // REMOVE n.property
                if let Expression::Property { object, key } = expr {
                    if let Expression::Variable(var) = object.as_ref() {
                        if let Some(node_val) = row.get(var) {
                            if let Some(node) = node_val.as_node() {
                                self.storage.remove_node_property(node.id, key)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // ─── ORDER BY ────────────────────────────────────────────────

    fn execute_order_by(
        &self,
        clause: &OrderByClause,
        mut rows: Vec<ResultRow>,
    ) -> Result<Vec<ResultRow>> {
        // We need to evaluate sort keys for each row
        let items = clause.items.clone();
        let self_ref = self;

        // Pre-compute sort keys to avoid borrowing issues
        let mut keyed_rows: Vec<(Vec<GraphValue>, ResultRow)> = rows
            .drain(..)
            .map(|row| {
                let keys: Vec<GraphValue> = items
                    .iter()
                    .map(|item| {
                        self_ref
                            .eval_expr(&item.expression, &row)
                            .unwrap_or(GraphValue::Null)
                    })
                    .collect();
                (keys, row)
            })
            .collect();

        keyed_rows.sort_by(|(keys_a, _), (keys_b, _)| {
            for (i, item) in items.iter().enumerate() {
                let a = &keys_a[i];
                let b = &keys_b[i];
                let ord = a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal);
                let ord = if item.descending { ord.reverse() } else { ord };
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            std::cmp::Ordering::Equal
        });

        Ok(keyed_rows.into_iter().map(|(_, row)| row).collect())
    }

    // ─── LIMIT / SKIP ───────────────────────────────────────────

    fn execute_limit(&self, clause: &LimitClause, rows: Vec<ResultRow>) -> Result<Vec<ResultRow>> {
        let count = match self.eval_expr(&clause.count, &ResultRow::new())? {
            GraphValue::Integer(n) => n as usize,
            _ => {
                return Err(VectraError::Graph {
                    message: "LIMIT requires integer".into(),
                })
            }
        };
        Ok(rows.into_iter().take(count).collect())
    }

    fn execute_skip(&self, clause: &SkipClause, rows: Vec<ResultRow>) -> Result<Vec<ResultRow>> {
        let count = match self.eval_expr(&clause.count, &ResultRow::new())? {
            GraphValue::Integer(n) => n as usize,
            _ => {
                return Err(VectraError::Graph {
                    message: "SKIP requires integer".into(),
                })
            }
        };
        Ok(rows.into_iter().skip(count).collect())
    }

    // ─── CALL ─────────────────────────────────────────────────────

    fn execute_call(&self, clause: &CallClause) -> Result<Vec<ResultRow>> {
        match clause.procedure.as_str() {
            "vectrust.nearest" => self.call_nearest(clause),
            other => Err(VectraError::Graph {
                message: format!("Unknown procedure: {}", other),
            }),
        }
    }

    /// CALL vectrust.nearest('field_name', $query_vector, k) YIELD node, score
    fn call_nearest(&self, clause: &CallClause) -> Result<Vec<ResultRow>> {
        // Parse arguments
        let empty_row = ResultRow::new();

        let _field_name = match clause.args.first() {
            Some(expr) => match self.eval_expr(expr, &empty_row)? {
                GraphValue::String(s) => s,
                _ => {
                    return Err(VectraError::Graph {
                        message: "vectrust.nearest: first argument must be a string (field name)"
                            .into(),
                    })
                }
            },
            None => {
                return Err(VectraError::Graph {
                    message: "vectrust.nearest requires 3 arguments: field_name, query_vector, k"
                        .into(),
                })
            }
        };

        let query_vec = match clause.args.get(1) {
            Some(expr) => {
                let val = self.eval_expr(expr, &empty_row)?;
                graphvalue_to_vec_f32(&val).ok_or_else(|| VectraError::Graph {
                    message: "vectrust.nearest: second argument must be a vector (list of floats)"
                        .into(),
                })?
            }
            None => {
                return Err(VectraError::Graph {
                    message: "vectrust.nearest requires 3 arguments".into(),
                })
            }
        };

        let k = match clause.args.get(2) {
            Some(expr) => match self.eval_expr(expr, &empty_row)? {
                GraphValue::Integer(n) => n as usize,
                _ => {
                    return Err(VectraError::Graph {
                        message: "vectrust.nearest: third argument must be an integer (k)".into(),
                    })
                }
            },
            None => 10, // default k
        };

        // Brute-force kNN: scan all node vectors and compute similarity
        let all_vectors = self.storage.all_node_vectors()?;
        let mut scored: Vec<(Uuid, f32)> = all_vectors
            .into_iter()
            .filter_map(|(id, vec)| {
                if vec.len() == query_vec.len() {
                    Some((id, cosine_similarity(&vec, &query_vec)))
                } else {
                    None
                }
            })
            .collect();

        // Sort by similarity descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        // Build result rows
        let node_var = clause
            .yields
            .iter()
            .find(|y| y.name == "node")
            .map(|y| y.alias.as_ref().unwrap_or(&y.name).clone())
            .unwrap_or_else(|| "node".to_string());
        let score_var = clause
            .yields
            .iter()
            .find(|y| y.name == "score")
            .map(|y| y.alias.as_ref().unwrap_or(&y.name).clone())
            .unwrap_or_else(|| "score".to_string());

        let mut rows = Vec::new();
        for (id, score) in scored {
            if let Some(node) = self.storage.get_node(id)? {
                let mut row = ResultRow::new();
                row.insert(node_var.clone(), GraphValue::Node(node));
                row.insert(score_var.clone(), GraphValue::Float(score as f64));
                rows.push(row);
            }
        }

        Ok(rows)
    }

    // ─── Expression evaluation ───────────────────────────────────

    fn eval_expr(&self, expr: &Expression, row: &ResultRow) -> Result<GraphValue> {
        match expr {
            Expression::Literal(lit) => Ok(match lit {
                Literal::Integer(n) => GraphValue::Integer(*n),
                Literal::Float(f) => GraphValue::Float(*f),
                Literal::String(s) => GraphValue::String(s.clone()),
                Literal::Bool(b) => GraphValue::Bool(*b),
                Literal::Null => GraphValue::Null,
            }),
            Expression::Variable(var) => Ok(row.get(var).cloned().unwrap_or(GraphValue::Null)),
            Expression::Property { object, key } => {
                // First try the flat key (e.g., "n.name" as a column after RETURN)
                let flat_key = format!("{}.{}", expr_name(object), key);
                if let Some(val) = row.get(&flat_key) {
                    return Ok(val.clone());
                }
                // Otherwise evaluate as property access on the object
                let obj = self.eval_expr(object, row)?;
                Ok(get_property(&obj, key))
            }
            Expression::Parameter(name) => {
                Ok(self.params.get(name).cloned().unwrap_or(GraphValue::Null))
            }
            Expression::Comparison { left, op, right } => {
                let l = self.eval_expr(left, row)?;
                let r = self.eval_expr(right, row)?;
                Ok(GraphValue::Bool(compare_values(&l, op, &r)))
            }
            Expression::BoolOp { left, op, right } => {
                let l = self.eval_expr(left, row)?;
                match op {
                    BooleanOp::And => {
                        if !is_truthy(&l) {
                            Ok(GraphValue::Bool(false))
                        } else {
                            let r = self.eval_expr(right, row)?;
                            Ok(GraphValue::Bool(is_truthy(&r)))
                        }
                    }
                    BooleanOp::Or => {
                        if is_truthy(&l) {
                            Ok(GraphValue::Bool(true))
                        } else {
                            let r = self.eval_expr(right, row)?;
                            Ok(GraphValue::Bool(is_truthy(&r)))
                        }
                    }
                }
            }
            Expression::Not(inner) => {
                let val = self.eval_expr(inner, row)?;
                Ok(GraphValue::Bool(!is_truthy(&val)))
            }
            Expression::IsNull {
                expression,
                negated,
            } => {
                let val = self.eval_expr(expression, row)?;
                let result = val.is_null();
                Ok(GraphValue::Bool(if *negated { !result } else { result }))
            }
            Expression::FunctionCall {
                name,
                args,
                distinct: _,
            } => self.eval_function(name, args, row),
            Expression::ListLiteral(elements) => {
                let values: Result<Vec<GraphValue>> =
                    elements.iter().map(|e| self.eval_expr(e, row)).collect();
                Ok(GraphValue::List(values?))
            }
            Expression::MapLiteral(map) => {
                let mut result = HashMap::new();
                for (key, value_expr) in &map.entries {
                    result.insert(key.clone(), self.eval_expr(value_expr, row)?);
                }
                Ok(GraphValue::Map(result))
            }
            Expression::StringOp { left, op, right } => {
                let l = self.eval_expr(left, row)?;
                let r = self.eval_expr(right, row)?;
                Ok(GraphValue::Bool(string_op(&l, op, &r)))
            }
            Expression::Arithmetic { left, op, right } => {
                let l = self.eval_expr(left, row)?;
                let r = self.eval_expr(right, row)?;
                Ok(arithmetic(&l, op, &r))
            }
            Expression::Star => Ok(GraphValue::Null), // count(*) handled in eval_function
        }
    }

    fn eval_function(
        &self,
        name: &str,
        args: &[Expression],
        row: &ResultRow,
    ) -> Result<GraphValue> {
        match name.to_lowercase().as_str() {
            "count" => {
                // count(*) or count(expr) — in row context, just return 1
                // Proper aggregation would need a separate pass
                Ok(GraphValue::Integer(1))
            }
            "vector_similarity" => {
                // vector_similarity(n.embedding, $query_vec)
                // First arg: property access on a node (we fetch the node's vector)
                // Second arg: query vector (list of floats or parameter)
                let node_vec = self.resolve_vector_arg(args.first(), row)?;
                let query_vec = self.resolve_vector_arg(args.get(1), row)?;
                match (node_vec, query_vec) {
                    (Some(a), Some(b)) if a.len() == b.len() => {
                        Ok(GraphValue::Float(cosine_similarity(&a, &b) as f64))
                    }
                    _ => Ok(GraphValue::Null),
                }
            }
            "vector_distance" => {
                let node_vec = self.resolve_vector_arg(args.first(), row)?;
                let query_vec = self.resolve_vector_arg(args.get(1), row)?;
                match (node_vec, query_vec) {
                    (Some(a), Some(b)) if a.len() == b.len() => {
                        Ok(GraphValue::Float(euclidean_distance(&a, &b) as f64))
                    }
                    _ => Ok(GraphValue::Null),
                }
            }
            "type" => {
                // type(r) returns the relationship type
                if let Some(arg) = args.first() {
                    let val = self.eval_expr(arg, row)?;
                    if let Some(edge) = val.as_edge() {
                        return Ok(GraphValue::String(edge.rel_type.clone()));
                    }
                }
                Ok(GraphValue::Null)
            }
            "id" => {
                if let Some(arg) = args.first() {
                    let val = self.eval_expr(arg, row)?;
                    match &val {
                        GraphValue::Node(n) => return Ok(GraphValue::String(n.id.to_string())),
                        GraphValue::Edge(e) => return Ok(GraphValue::String(e.id.to_string())),
                        _ => {}
                    }
                }
                Ok(GraphValue::Null)
            }
            "labels" => {
                if let Some(arg) = args.first() {
                    let val = self.eval_expr(arg, row)?;
                    if let Some(node) = val.as_node() {
                        return Ok(GraphValue::List(
                            node.labels
                                .iter()
                                .map(|l| GraphValue::String(l.clone()))
                                .collect(),
                        ));
                    }
                }
                Ok(GraphValue::Null)
            }
            "toInteger" | "tointeger" => {
                if let Some(arg) = args.first() {
                    let val = self.eval_expr(arg, row)?;
                    return match val {
                        GraphValue::Integer(_) => Ok(val),
                        GraphValue::Float(f) => Ok(GraphValue::Integer(f as i64)),
                        GraphValue::String(s) => {
                            s.parse::<i64>().map(GraphValue::Integer).map_err(|_| {
                                VectraError::Graph {
                                    message: format!("Cannot convert '{}' to integer", s),
                                }
                            })
                        }
                        _ => Ok(GraphValue::Null),
                    };
                }
                Ok(GraphValue::Null)
            }
            "toString" | "tostring" => {
                if let Some(arg) = args.first() {
                    let val = self.eval_expr(arg, row)?;
                    return Ok(GraphValue::String(format_value(&val)));
                }
                Ok(GraphValue::Null)
            }
            _ => Err(VectraError::Graph {
                message: format!("Unknown function: {}", name),
            }),
        }
    }

    fn eval_map_properties(
        &self,
        map: Option<&vectrust_cypher::MapLiteral>,
        bindings: &ResultRow,
    ) -> Result<HashMap<String, GraphValue>> {
        match map {
            Some(m) => {
                let mut props = HashMap::new();
                for (key, expr) in &m.entries {
                    props.insert(key.clone(), self.eval_expr(expr, bindings)?);
                }
                Ok(props)
            }
            None => Ok(HashMap::new()),
        }
    }

    /// Resolve a vector function argument to a Vec<f32>.
    /// Handles:
    /// - Property access on a node (fetches the node's stored vector)
    /// - Parameter reference (expects list of floats)
    /// - List literal
    fn resolve_vector_arg(
        &self,
        arg: Option<&Expression>,
        row: &ResultRow,
    ) -> Result<Option<Vec<f32>>> {
        let Some(arg) = arg else { return Ok(None) };

        // If it's a property access like n.embedding, get the node's stored vector
        if let Expression::Property { object, .. } = arg {
            if let Expression::Variable(var) = object.as_ref() {
                if let Some(node_val) = row.get(var) {
                    if let Some(node) = node_val.as_node() {
                        return self.storage.get_node_vector(node.id);
                    }
                }
            }
        }

        // Otherwise evaluate the expression and try to extract a float list
        let val = self.eval_expr(arg, row)?;
        Ok(graphvalue_to_vec_f32(&val))
    }
}

// ─── Helper functions ────────────────────────────────────────────

fn get_property(value: &GraphValue, key: &str) -> GraphValue {
    match value {
        GraphValue::Node(node) => node
            .properties
            .get(key)
            .cloned()
            .unwrap_or(GraphValue::Null),
        GraphValue::Edge(edge) => edge
            .properties
            .get(key)
            .cloned()
            .unwrap_or(GraphValue::Null),
        GraphValue::Map(map) => map.get(key).cloned().unwrap_or(GraphValue::Null),
        _ => GraphValue::Null,
    }
}

fn is_truthy(value: &GraphValue) -> bool {
    match value {
        GraphValue::Bool(b) => *b,
        GraphValue::Null => false,
        GraphValue::Integer(0) => false,
        GraphValue::String(s) if s.is_empty() => false,
        _ => true,
    }
}

fn compare_values(left: &GraphValue, op: &ComparisonOp, right: &GraphValue) -> bool {
    match op {
        ComparisonOp::Eq => left == right,
        ComparisonOp::Neq => left != right,
        ComparisonOp::Lt => left.partial_cmp(right) == Some(std::cmp::Ordering::Less),
        ComparisonOp::Gt => left.partial_cmp(right) == Some(std::cmp::Ordering::Greater),
        ComparisonOp::Lte => matches!(
            left.partial_cmp(right),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        ),
        ComparisonOp::Gte => matches!(
            left.partial_cmp(right),
            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        ),
    }
}

fn string_op(left: &GraphValue, op: &StringMatchOp, right: &GraphValue) -> bool {
    match op {
        StringMatchOp::Contains => {
            if let (Some(l), Some(r)) = (left.as_str(), right.as_str()) {
                l.contains(r)
            } else {
                false
            }
        }
        StringMatchOp::StartsWith => {
            if let (Some(l), Some(r)) = (left.as_str(), right.as_str()) {
                l.starts_with(r)
            } else {
                false
            }
        }
        StringMatchOp::EndsWith => {
            if let (Some(l), Some(r)) = (left.as_str(), right.as_str()) {
                l.ends_with(r)
            } else {
                false
            }
        }
        StringMatchOp::In => {
            if let GraphValue::List(list) = right {
                list.iter().any(|item| item == left)
            } else {
                false
            }
        }
    }
}

fn arithmetic(left: &GraphValue, op: &ArithmeticOp, right: &GraphValue) -> GraphValue {
    match (left, right) {
        (GraphValue::Integer(a), GraphValue::Integer(b)) => match op {
            ArithmeticOp::Add => GraphValue::Integer(a + b),
            ArithmeticOp::Subtract => GraphValue::Integer(a - b),
            ArithmeticOp::Multiply => GraphValue::Integer(a * b),
            ArithmeticOp::Divide => {
                if *b == 0 {
                    GraphValue::Null
                } else {
                    GraphValue::Integer(a / b)
                }
            }
            ArithmeticOp::Modulo => {
                if *b == 0 {
                    GraphValue::Null
                } else {
                    GraphValue::Integer(a % b)
                }
            }
        },
        (GraphValue::Float(a), GraphValue::Float(b)) => match op {
            ArithmeticOp::Add => GraphValue::Float(a + b),
            ArithmeticOp::Subtract => GraphValue::Float(a - b),
            ArithmeticOp::Multiply => GraphValue::Float(a * b),
            ArithmeticOp::Divide => GraphValue::Float(a / b),
            ArithmeticOp::Modulo => GraphValue::Float(a % b),
        },
        (GraphValue::Integer(a), GraphValue::Float(b)) => {
            arithmetic(&GraphValue::Float(*a as f64), op, &GraphValue::Float(*b))
        }
        (GraphValue::Float(a), GraphValue::Integer(b)) => {
            arithmetic(&GraphValue::Float(*a), op, &GraphValue::Float(*b as f64))
        }
        (GraphValue::String(a), GraphValue::String(b)) if matches!(op, ArithmeticOp::Add) => {
            GraphValue::String(format!("{}{}", a, b))
        }
        _ => GraphValue::Null,
    }
}

/// Generate a display name for an expression (used as default column name).
fn expr_name(expr: &Expression) -> String {
    match expr {
        Expression::Variable(name) => name.clone(),
        Expression::Property { object, key } => format!("{}.{}", expr_name(object), key),
        Expression::FunctionCall { name, .. } => format!("{}(..)", name),
        Expression::Star => "*".to_string(),
        _ => "expr".to_string(),
    }
}

fn format_value(val: &GraphValue) -> String {
    match val {
        GraphValue::Null => "null".to_string(),
        GraphValue::Bool(b) => b.to_string(),
        GraphValue::Integer(n) => n.to_string(),
        GraphValue::Float(f) => f.to_string(),
        GraphValue::String(s) => s.clone(),
        _ => format!("{:?}", val),
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f32>()
        .sqrt()
}

fn graphvalue_to_vec_f32(val: &GraphValue) -> Option<Vec<f32>> {
    match val {
        GraphValue::List(items) => {
            let floats: Option<Vec<f32>> =
                items.iter().map(|v| v.as_f64().map(|f| f as f32)).collect();
            floats
        }
        _ => None,
    }
}

/// Check if an expression is an aggregate function (count, collect, sum, avg, etc.)
fn is_aggregate_expr(expr: &Expression) -> bool {
    match expr {
        Expression::FunctionCall { name, .. } => {
            matches!(
                name.to_lowercase().as_str(),
                "count" | "collect" | "sum" | "avg" | "min" | "max"
            )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::GraphStorage;
    use tempfile::TempDir;
    use vectrust_cypher::parse;

    fn setup() -> (GraphStorage, TempDir) {
        let dir = TempDir::new().unwrap();
        let storage = GraphStorage::open(dir.path()).unwrap();
        (storage, dir)
    }

    fn exec(storage: &GraphStorage, cypher: &str) -> GraphQueryResult {
        exec_with_params(storage, cypher, HashMap::new())
    }

    fn exec_with_params(
        storage: &GraphStorage,
        cypher: &str,
        params: HashMap<String, GraphValue>,
    ) -> GraphQueryResult {
        let stmt = parse(cypher).expect("parse failed");
        let executor = GraphExecutor::new(storage, params);
        executor.execute(&stmt).expect("execute failed")
    }

    #[test]
    fn test_create_and_match_node() {
        let (storage, _dir) = setup();

        // Create a node
        exec(&storage, "CREATE (n:Person {name: 'Alice', age: 30})");

        // Match it back
        let result = exec(&storage, "MATCH (n:Person) RETURN n.name, n.age");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("n.name"),
            Some(&GraphValue::String("Alice".into()))
        );
        assert_eq!(result.rows[0].get("n.age"), Some(&GraphValue::Integer(30)));
    }

    #[test]
    fn test_create_and_traverse_edge() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (a:Person {name: 'Alice'})");
        exec(&storage, "CREATE (b:Person {name: 'Bob'})");

        // Create edge via MATCH + CREATE
        exec(
            &storage,
            "MATCH (a:Person), (b:Person) WHERE a.name = 'Alice' AND b.name = 'Bob' CREATE (a)-[:KNOWS {since: 2020}]->(b)",
        );

        // Traverse
        let result = exec(
            &storage,
            "MATCH (a:Person)-[:KNOWS]->(b:Person) WHERE a.name = 'Alice' RETURN b.name",
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("b.name"),
            Some(&GraphValue::String("Bob".into()))
        );
    }

    #[test]
    fn test_where_filter() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Person {name: 'Alice', age: 30})");
        exec(&storage, "CREATE (n:Person {name: 'Bob', age: 25})");
        exec(&storage, "CREATE (n:Person {name: 'Carol', age: 35})");

        let result = exec(
            &storage,
            "MATCH (n:Person) WHERE n.age > 28 RETURN n.name ORDER BY n.name",
        );
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_order_by_limit() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Person {name: 'Alice', age: 30})");
        exec(&storage, "CREATE (n:Person {name: 'Bob', age: 25})");
        exec(&storage, "CREATE (n:Person {name: 'Carol', age: 35})");

        let result = exec(
            &storage,
            "MATCH (n:Person) RETURN n.name, n.age ORDER BY n.age DESC LIMIT 2",
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            result.rows[0].get("n.name"),
            Some(&GraphValue::String("Carol".into()))
        );
        assert_eq!(
            result.rows[1].get("n.name"),
            Some(&GraphValue::String("Alice".into()))
        );
    }

    #[test]
    fn test_parameter_binding() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Person {name: 'Alice', age: 30})");
        exec(&storage, "CREATE (n:Person {name: 'Bob', age: 25})");

        let mut params = HashMap::new();
        params.insert("min_age".to_string(), GraphValue::Integer(28));

        let result = exec_with_params(
            &storage,
            "MATCH (n:Person) WHERE n.age > $min_age RETURN n.name",
            params,
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("n.name"),
            Some(&GraphValue::String("Alice".into()))
        );
    }

    #[test]
    fn test_set_property() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Person {name: 'Alice', age: 30})");
        exec(
            &storage,
            "MATCH (n:Person) WHERE n.name = 'Alice' SET n.age = 31",
        );

        let result = exec(
            &storage,
            "MATCH (n:Person) WHERE n.name = 'Alice' RETURN n.age",
        );
        assert_eq!(result.rows[0].get("n.age"), Some(&GraphValue::Integer(31)));
    }

    #[test]
    fn test_delete_node() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Person {name: 'Alice'})");
        exec(
            &storage,
            "MATCH (n:Person) WHERE n.name = 'Alice' DETACH DELETE n",
        );

        let result = exec(&storage, "MATCH (n:Person) RETURN n");
        assert_eq!(result.rows.len(), 0);
    }

    #[test]
    fn test_return_alias() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Person {name: 'Alice'})");
        let result = exec(&storage, "MATCH (n:Person) RETURN n.name AS name");
        assert_eq!(result.columns, vec!["name"]);
        assert_eq!(
            result.rows[0].get("name"),
            Some(&GraphValue::String("Alice".into()))
        );
    }

    #[test]
    fn test_contains_filter() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Person {name: 'Alice Johnson'})");
        exec(&storage, "CREATE (n:Person {name: 'Bob Smith'})");

        let result = exec(
            &storage,
            "MATCH (n:Person) WHERE n.name CONTAINS 'Johnson' RETURN n.name",
        );
        assert_eq!(result.rows.len(), 1);
    }

    #[test]
    fn test_combined_prd_query() {
        let (storage, _dir) = setup();

        // Set up graph
        exec(
            &storage,
            "CREATE (d:Document {title: 'AI Overview', topic: 'AI'})",
        );
        exec(
            &storage,
            "CREATE (r:Document {title: 'Deep Learning', topic: 'AI'})",
        );
        exec(
            &storage,
            "CREATE (x:Document {title: 'Cooking', topic: 'Food'})",
        );

        exec(
            &storage,
            "MATCH (d:Document), (r:Document) WHERE d.title = 'AI Overview' AND r.title = 'Deep Learning' CREATE (d)-[:REFERENCES]->(r)",
        );

        // The PRD query pattern (without vector functions for now)
        let result = exec(
            &storage,
            "MATCH (doc:Document)-[:REFERENCES]->(ref:Document) WHERE doc.topic = 'AI' RETURN ref.title",
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("ref.title"),
            Some(&GraphValue::String("Deep Learning".into()))
        );
    }

    #[test]
    fn test_variable_length_path() {
        let (storage, _dir) = setup();

        // Build a chain: A -> B -> C -> D
        exec(&storage, "CREATE (a:Node {name: 'A'})");
        exec(&storage, "CREATE (b:Node {name: 'B'})");
        exec(&storage, "CREATE (c:Node {name: 'C'})");
        exec(&storage, "CREATE (d:Node {name: 'D'})");

        exec(
            &storage,
            "MATCH (a:Node), (b:Node) WHERE a.name = 'A' AND b.name = 'B' CREATE (a)-[:NEXT]->(b)",
        );
        exec(
            &storage,
            "MATCH (b:Node), (c:Node) WHERE b.name = 'B' AND c.name = 'C' CREATE (b)-[:NEXT]->(c)",
        );
        exec(
            &storage,
            "MATCH (c:Node), (d:Node) WHERE c.name = 'C' AND d.name = 'D' CREATE (c)-[:NEXT]->(d)",
        );

        // Exactly 1 hop from A: should find B
        let result = exec(
            &storage,
            "MATCH (a:Node)-[:NEXT*1..1]->(b:Node) WHERE a.name = 'A' RETURN b.name",
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("b.name"),
            Some(&GraphValue::String("B".into()))
        );

        // 1..2 hops from A: should find B and C
        let result = exec(&storage, "MATCH (a:Node)-[:NEXT*1..2]->(b:Node) WHERE a.name = 'A' RETURN b.name ORDER BY b.name");
        assert_eq!(result.rows.len(), 2);

        // 1..3 hops from A: should find B, C, and D
        let result = exec(&storage, "MATCH (a:Node)-[:NEXT*1..3]->(b:Node) WHERE a.name = 'A' RETURN b.name ORDER BY b.name");
        assert_eq!(result.rows.len(), 3);

        // 2..3 hops from A: should find C and D (not B)
        let result = exec(&storage, "MATCH (a:Node)-[:NEXT*2..3]->(b:Node) WHERE a.name = 'A' RETURN b.name ORDER BY b.name");
        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            result.rows[0].get("b.name"),
            Some(&GraphValue::String("C".into()))
        );
        assert_eq!(
            result.rows[1].get("b.name"),
            Some(&GraphValue::String("D".into()))
        );
    }

    #[test]
    fn test_variable_length_no_cycles() {
        let (storage, _dir) = setup();

        // Build a cycle: A -> B -> A
        exec(&storage, "CREATE (a:Node {name: 'A'})");
        exec(&storage, "CREATE (b:Node {name: 'B'})");
        exec(
            &storage,
            "MATCH (a:Node), (b:Node) WHERE a.name = 'A' AND b.name = 'B' CREATE (a)-[:LINK]->(b)",
        );
        exec(
            &storage,
            "MATCH (a:Node), (b:Node) WHERE b.name = 'A' AND a.name = 'B' CREATE (a)-[:LINK]->(b)",
        );

        // Should not loop infinitely; cycle detection prevents revisiting A
        let result = exec(
            &storage,
            "MATCH (a:Node)-[:LINK*1..5]->(b:Node) WHERE a.name = 'A' RETURN b.name",
        );
        // Should find B (1 hop) and A (2 hops via B->A, but A is the source so skipped by cycle detection)
        // Only B reachable without revisiting
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("b.name"),
            Some(&GraphValue::String("B".into()))
        );
    }

    #[test]
    fn test_count_star() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Person {name: 'Alice'})");
        exec(&storage, "CREATE (n:Person {name: 'Bob'})");
        exec(&storage, "CREATE (n:Person {name: 'Carol'})");

        let result = exec(&storage, "MATCH (n:Person) RETURN count(*) AS total");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].get("total"), Some(&GraphValue::Integer(3)));
    }

    #[test]
    fn test_count_with_grouping() {
        let (storage, _dir) = setup();

        exec(
            &storage,
            "CREATE (n:Person {name: 'Alice', dept: 'Engineering'})",
        );
        exec(
            &storage,
            "CREATE (n:Person {name: 'Bob', dept: 'Engineering'})",
        );
        exec(
            &storage,
            "CREATE (n:Person {name: 'Carol', dept: 'Marketing'})",
        );

        let result = exec(
            &storage,
            "MATCH (n:Person) RETURN n.dept AS dept, count(*) AS total ORDER BY dept",
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            result.rows[0].get("dept"),
            Some(&GraphValue::String("Engineering".into()))
        );
        assert_eq!(result.rows[0].get("total"), Some(&GraphValue::Integer(2)));
        assert_eq!(
            result.rows[1].get("dept"),
            Some(&GraphValue::String("Marketing".into()))
        );
        assert_eq!(result.rows[1].get("total"), Some(&GraphValue::Integer(1)));
    }

    #[test]
    fn test_collect_aggregation() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Person {name: 'Alice', dept: 'Eng'})");
        exec(&storage, "CREATE (n:Person {name: 'Bob', dept: 'Eng'})");
        exec(&storage, "CREATE (n:Person {name: 'Carol', dept: 'Mkt'})");

        let result = exec(
            &storage,
            "MATCH (n:Person) RETURN n.dept AS dept, collect(n.name) AS names ORDER BY dept",
        );
        assert_eq!(result.rows.len(), 2);
        // Eng department should have 2 names collected
        if let Some(GraphValue::List(names)) = result.rows[0].get("names") {
            assert_eq!(names.len(), 2);
        } else {
            panic!("Expected list for collect()");
        }
    }

    #[test]
    fn test_vector_similarity() {
        let (storage, _dir) = setup();

        // Create nodes with vectors
        storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("AI Paper".into()))]),
                vec![1.0, 0.0, 0.0],
            )
            .unwrap();

        storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("ML Paper".into()))]),
                vec![0.9, 0.1, 0.0],
            )
            .unwrap();

        storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("Cooking".into()))]),
                vec![0.0, 0.0, 1.0],
            )
            .unwrap();

        // Query with vector similar to first doc
        let mut params = HashMap::new();
        params.insert(
            "query".to_string(),
            GraphValue::List(vec![
                GraphValue::Float(1.0),
                GraphValue::Float(0.0),
                GraphValue::Float(0.0),
            ]),
        );

        let result = exec_with_params(
            &storage,
            "MATCH (n:Doc) RETURN n.title AS title, vector_similarity(n.embedding, $query) AS score ORDER BY score DESC",
            params,
        );

        assert_eq!(result.rows.len(), 3);
        // First result should be "AI Paper" (exact match = 1.0 similarity)
        assert_eq!(
            result.rows[0].get("title"),
            Some(&GraphValue::String("AI Paper".into()))
        );
        // Score should be 1.0 for exact match
        if let Some(GraphValue::Float(score)) = result.rows[0].get("score") {
            assert!(*score > 0.99, "Expected ~1.0, got {}", score);
        } else {
            panic!("Expected float score");
        }
        // Last result should be "Cooking" (orthogonal = 0.0 similarity)
        assert_eq!(
            result.rows[2].get("title"),
            Some(&GraphValue::String("Cooking".into()))
        );
    }

    #[test]
    fn test_vector_distance_in_where() {
        let (storage, _dir) = setup();

        storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("Close".into()))]),
                vec![1.0, 0.0, 0.0],
            )
            .unwrap();

        storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("Far".into()))]),
                vec![0.0, 1.0, 0.0],
            )
            .unwrap();

        let mut params = HashMap::new();
        params.insert(
            "q".to_string(),
            GraphValue::List(vec![
                GraphValue::Float(1.0),
                GraphValue::Float(0.0),
                GraphValue::Float(0.0),
            ]),
        );

        // Filter by similarity threshold
        let result = exec_with_params(
            &storage,
            "MATCH (n:Doc) WHERE vector_similarity(n.embedding, $q) > 0.5 RETURN n.title AS title",
            params,
        );

        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("title"),
            Some(&GraphValue::String("Close".into()))
        );
    }

    #[test]
    fn test_call_nearest() {
        let (storage, _dir) = setup();

        // Create nodes with vectors
        storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("AI".into()))]),
                vec![1.0, 0.0, 0.0],
            )
            .unwrap();
        storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("ML".into()))]),
                vec![0.9, 0.1, 0.0],
            )
            .unwrap();
        storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("Cook".into()))]),
                vec![0.0, 0.0, 1.0],
            )
            .unwrap();

        let mut params = HashMap::new();
        params.insert(
            "query".to_string(),
            GraphValue::List(vec![
                GraphValue::Float(1.0),
                GraphValue::Float(0.0),
                GraphValue::Float(0.0),
            ]),
        );

        let result = exec_with_params(
            &storage,
            "CALL vectrust.nearest('embedding', $query, 2) YIELD node, score RETURN node.title AS title, score",
            params,
        );

        assert_eq!(result.rows.len(), 2);
        // First result should be exact match
        assert_eq!(
            result.rows[0].get("title"),
            Some(&GraphValue::String("AI".into()))
        );
        // Score should be ~1.0
        if let Some(GraphValue::Float(s)) = result.rows[0].get("score") {
            assert!(*s > 0.99);
        }
    }

    #[test]
    fn test_call_nearest_with_graph_traversal() {
        let (storage, _dir) = setup();

        // Create nodes with vectors and relationships
        let ai_id = storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("AI Paper".into()))]),
                vec![1.0, 0.0, 0.0],
            )
            .unwrap();
        let author_id = storage
            .create_node(
                &["Person".to_string()],
                HashMap::from([("name".to_string(), GraphValue::String("Alice".into()))]),
            )
            .unwrap();
        storage
            .create_edge(author_id, ai_id, "AUTHORED", HashMap::new())
            .unwrap();

        storage
            .create_node_with_vector(
                &["Doc".to_string()],
                HashMap::from([("title".to_string(), GraphValue::String("Cooking".into()))]),
                vec![0.0, 0.0, 1.0],
            )
            .unwrap();

        let mut params = HashMap::new();
        params.insert(
            "query".to_string(),
            GraphValue::List(vec![
                GraphValue::Float(1.0),
                GraphValue::Float(0.0),
                GraphValue::Float(0.0),
            ]),
        );

        // kNN + graph traversal: find nearest docs then traverse to author
        let result = exec_with_params(
            &storage,
            "CALL vectrust.nearest('embedding', $query, 1) YIELD node, score \
             MATCH (p:Person)-[:AUTHORED]->(node) \
             RETURN p.name AS author, node.title AS doc, score",
            params,
        );

        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("author"),
            Some(&GraphValue::String("Alice".into()))
        );
        assert_eq!(
            result.rows[0].get("doc"),
            Some(&GraphValue::String("AI Paper".into()))
        );
    }

    #[test]
    fn test_distinct() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Item {category: 'A'})");
        exec(&storage, "CREATE (n:Item {category: 'A'})");
        exec(&storage, "CREATE (n:Item {category: 'B'})");
        exec(&storage, "CREATE (n:Item {category: 'A'})");

        let result = exec(
            &storage,
            "MATCH (n:Item) RETURN DISTINCT n.category AS cat ORDER BY cat",
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            result.rows[0].get("cat"),
            Some(&GraphValue::String("A".into()))
        );
        assert_eq!(
            result.rows[1].get("cat"),
            Some(&GraphValue::String("B".into()))
        );
    }

    #[test]
    fn test_remove_property() {
        let (storage, _dir) = setup();

        exec(
            &storage,
            "CREATE (n:Person {name: 'Alice', temp: 'delete_me'})",
        );

        // Verify property exists
        let result = exec(&storage, "MATCH (n:Person) RETURN n.temp AS t");
        assert_eq!(
            result.rows[0].get("t"),
            Some(&GraphValue::String("delete_me".into()))
        );

        // Remove it
        exec(&storage, "MATCH (n:Person) REMOVE n.temp");

        // Should be null now
        let result = exec(&storage, "MATCH (n:Person) RETURN n.temp AS t");
        assert_eq!(result.rows[0].get("t"), Some(&GraphValue::Null));
    }

    #[test]
    fn test_sum_avg_min_max() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Score {val: 10})");
        exec(&storage, "CREATE (n:Score {val: 20})");
        exec(&storage, "CREATE (n:Score {val: 30})");

        let result = exec(&storage, "MATCH (n:Score) RETURN sum(n.val) AS s, avg(n.val) AS a, min(n.val) AS lo, max(n.val) AS hi");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].get("s"), Some(&GraphValue::Integer(60)));
        assert_eq!(result.rows[0].get("a"), Some(&GraphValue::Float(20.0)));
        assert_eq!(result.rows[0].get("lo"), Some(&GraphValue::Integer(10)));
        assert_eq!(result.rows[0].get("hi"), Some(&GraphValue::Integer(30)));
    }

    #[test]
    fn test_sum_with_grouping() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (n:Sale {dept: 'A', amount: 100})");
        exec(&storage, "CREATE (n:Sale {dept: 'A', amount: 200})");
        exec(&storage, "CREATE (n:Sale {dept: 'B', amount: 50})");

        let result = exec(
            &storage,
            "MATCH (n:Sale) RETURN n.dept AS dept, sum(n.amount) AS total ORDER BY dept",
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].get("total"), Some(&GraphValue::Integer(300)));
        assert_eq!(result.rows[1].get("total"), Some(&GraphValue::Integer(50)));
    }

    #[test]
    fn test_delete_edge() {
        let (storage, _dir) = setup();

        exec(&storage, "CREATE (a:Person {name: 'Alice'})");
        exec(&storage, "CREATE (b:Person {name: 'Bob'})");
        exec(&storage, "MATCH (a:Person), (b:Person) WHERE a.name = 'Alice' AND b.name = 'Bob' CREATE (a)-[:KNOWS]->(b)");

        // Verify edge exists
        let result = exec(
            &storage,
            "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a.name",
        );
        assert_eq!(result.rows.len(), 1);

        // Delete the edge (not the nodes)
        exec(&storage, "MATCH (a:Person)-[r:KNOWS]->(b:Person) DELETE r");

        // Edge gone, nodes remain
        let result = exec(
            &storage,
            "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a.name",
        );
        assert_eq!(result.rows.len(), 0);

        let result = exec(&storage, "MATCH (n:Person) RETURN n.name ORDER BY n.name");
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_count_empty_set() {
        let (storage, _dir) = setup();

        // count(*) on empty match should return 0
        let result = exec(&storage, "MATCH (n:Nothing) RETURN count(*) AS total");
        // With no matching rows, the aggregation should still produce a row with 0
        // (this is standard SQL/Cypher behavior)
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].get("total"), Some(&GraphValue::Integer(0)));
    }
}
