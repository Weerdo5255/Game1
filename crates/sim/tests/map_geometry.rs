use sim::content::*;
use sim::*;

fn pos(n: NodeId) -> (f64, f64) {
    let d = &NODES[n.index()];
    (d.x, d.y)
}

fn cross(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

fn segments_cross(p1: (f64, f64), p2: (f64, f64), q1: (f64, f64), q2: (f64, f64)) -> bool {
    let (d1, d2) = (cross(q1, q2, p1), cross(q1, q2, p2));
    let (d3, d4) = (cross(p1, p2, q1), cross(p1, p2, q2));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn point_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).max(0.0).min(1.0);
    let (cx, cy) = (a.0 + t * dx - p.0, a.1 + t * dy - p.1);
    (cx * cx + cy * cy).sqrt()
}

#[test]
fn map_geometry() {
    let t = &TUNING;
    assert_eq!(LINKS.iter().filter(|l| l.kind == LinkKind::Cluster).count(), 20);
    assert_eq!(LINKS.iter().filter(|l| l.kind == LinkKind::Spur).count(), 11);
    for l in LINKS.iter() {
        let len = link_length(l.a, l.b).unwrap();
        match l.kind {
            LinkKind::Spur => assert!(len >= t.spur_min, "spur {:?}-{:?} is {len}", l.a, l.b),
            LinkKind::Cluster => assert!((94.5..=116.0).contains(&len), "cluster {:?}-{:?} is {len}", l.a, l.b),
            _ => {}
        }
    }
    for u in UNITS.iter() {
        assert!(u.speed <= t.max_unit_speed);
    }
    for (i, a) in LINKS.iter().enumerate() {
        for b in LINKS.iter().skip(i + 1) {
            if a.a == b.a || a.a == b.b || a.b == b.a || a.b == b.b {
                continue;
            }
            assert!(!segments_cross(pos(a.a), pos(a.b), pos(b.a), pos(b.b)), "{a:?} crosses {b:?}");
        }
    }
    for a in NODES.iter() {
        for b in NODES.iter().filter(|b| b.id > a.id) {
            let d = ((a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)).sqrt();
            assert!(d >= 73.0, "{} and {} are {d} apart", a.name, b.name);
        }
        for l in LINKS.iter().filter(|l| l.a != a.id && l.b != a.id) {
            let d = point_segment((a.x, a.y), pos(l.a), pos(l.b));
            assert!(d >= 40.0, "{} is {d} from link {:?}-{:?}", a.name, l.a, l.b);
        }
    }
    for &n in neighbours(NodeId(0)) {
        assert_ne!(NODES[n.index()].kind, NodeKind::Stronghold);
    }
    assert_eq!(NODES.iter().map(|d| weight_of(d.kind)).sum::<u32>(), 100);
    for d in NODES.iter() {
        let leaf = matches!(d.kind, NodeKind::Stronghold | NodeKind::Lab);
        assert_eq!(d.approach.is_some(), leaf);
        if let Some(a) = d.approach {
            assert_eq!(neighbours(d.id), &[a]);
            assert!(link_length(d.id, a).is_some());
            assert!(matches!(NODES[a.index()].kind, NodeKind::City | NodeKind::Datacentre | NodeKind::Exchange));
        }
    }
}

#[test]
fn neighbours_match_links() {
    for d in NODES.iter() {
        let mut expect: Vec<NodeId> = LINKS
            .iter()
            .filter_map(|l| if l.a == d.id { Some(l.b) } else if l.b == d.id { Some(l.a) } else { None })
            .collect();
        expect.sort();
        assert_eq!(neighbours(d.id), expect.as_slice(), "node {}", d.id.0);
    }
}

#[test]
fn routes_to_core_match_reference() {
    let g = Game::new(1);
    let ids = |v: &[u8]| v.iter().map(|&n| NodeId(n)).collect::<Vec<_>>();
    let cases: [(&[u8], f64); 11] = [
        (&[4, 3, 1, 0], 350.0),
        (&[8, 6, 5, 2, 0], 484.0),
        (&[9, 5, 2, 0], 378.0),
        (&[13, 11, 10, 3, 1, 0], 580.0),
        (&[14, 11, 10, 3, 1, 0], 583.0),
        (&[19, 15, 7, 5, 2, 0], 674.0),
        (&[18, 17, 15, 7, 5, 2, 0], 778.0),
        (&[24, 20, 12, 10, 3, 1, 0], 777.0),
        (&[23, 21, 20, 12, 10, 3, 1, 0], 879.0),
        (&[28, 25, 17, 15, 7, 5, 2, 0], 993.0),
        (&[29, 27, 25, 17, 15, 7, 5, 2, 0], 1093.0),
    ];
    for (path, len) in cases {
        let route = g.route(NodeId(path[0]), NodeId(0));
        assert_eq!(route, ids(path));
        let total: f64 = route.windows(2).map(|w| link_length(w[0], w[1]).unwrap()).sum();
        assert!((total - len).abs() < 2.5, "route from {} is {total}, expected {len}", path[0]);
    }
}
