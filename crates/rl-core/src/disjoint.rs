//! Union-find over dense indices.
//!
//! Region merging during generation, road-network building (Kruskal) and
//! choke-point analysis all need "are these two in the same group yet".

/// A disjoint-set forest with path halving and union by size.
#[derive(Debug, Clone)]
pub struct DisjointSet {
    parent: Vec<usize>,
    size: Vec<usize>,
    groups: usize,
}

impl DisjointSet {
    /// `n` singleton groups.
    pub fn new(n: usize) -> Self {
        Self { parent: (0..n).collect(), size: vec![1; n], groups: n }
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Whether there are no elements.
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Number of distinct groups.
    pub fn groups(&self) -> usize {
        self.groups
    }

    /// The representative of `x`'s group.
    pub fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Merges the groups of `a` and `b`. Returns `true` if they were separate.
    pub fn union(&mut self, a: usize, b: usize) -> bool {
        let (mut ra, mut rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        if self.size[ra] < self.size[rb] {
            std::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        self.size[ra] += self.size[rb];
        self.groups -= 1;
        true
    }

    /// Whether `a` and `b` are in the same group.
    pub fn same(&mut self, a: usize, b: usize) -> bool {
        self.find(a) == self.find(b)
    }

    /// Size of `x`'s group.
    pub fn group_size(&mut self, x: usize) -> usize {
        let r = self.find(x);
        self.size[r]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singletons_then_merges() {
        let mut ds = DisjointSet::new(5);
        assert_eq!(ds.groups(), 5);
        assert!(ds.union(0, 1));
        assert!(ds.union(3, 4));
        assert!(!ds.union(1, 0));
        assert_eq!(ds.groups(), 3);
        assert!(ds.same(0, 1));
        assert!(!ds.same(1, 3));
        assert!(ds.union(1, 4));
        assert!(ds.same(0, 3));
        assert_eq!(ds.group_size(2), 1);
        assert_eq!(ds.group_size(4), 4);
    }
}
