use super::indexed::{DefaultDomain, IndexSet, IndexedDomain, IndexedValue};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_index::vec::Enumerated;
use rustc_middle::{
  mir::{Body, Local, Location, Place, ProjectionElem},
  ty::TyCtxt,
};

use std::{cell::RefCell, rc::Rc, slice::Iter};

rustc_index::newtype_index! {
  pub struct PlaceIndex {
      DEBUG_FORMAT = "p{}"
  }
}

struct NormalizedPlaces<'tcx> {
  tcx: TyCtxt<'tcx>,
  cache: HashMap<Place<'tcx>, Place<'tcx>>,
}

impl NormalizedPlaces<'tcx> {
  fn normalize(&mut self, place: Place<'tcx>) -> Place<'tcx> {
    let tcx = self.tcx;
    *self.cache.entry(place).or_insert_with(|| {
      let place = tcx.erase_regions(place);
      let projection = place
        .projection
        .into_iter()
        .map(|elem| match elem {
          ProjectionElem::Index(_) => ProjectionElem::Index(Local::from_usize(0)),
          _ => elem,
        })
        .collect::<Vec<_>>();

      Place {
        local: place.local,
        projection: tcx.intern_place_elems(&projection),
      }
    })
  }
}

#[derive(Clone)]
pub struct PlaceDomain<'tcx> {
  domain: DefaultDomain<PlaceIndex, Place<'tcx>>,
  normalized_places: Rc<RefCell<NormalizedPlaces<'tcx>>>,
}

impl PlaceDomain<'tcx> {
  pub fn new(tcx: TyCtxt<'tcx>, places: Vec<Place<'tcx>>) -> Self {
    let normalized_places = Rc::new(RefCell::new(NormalizedPlaces {
      tcx,
      cache: HashMap::default(),
    }));

    let domain = DefaultDomain::new(
      places
        .into_iter()
        .map(|place| normalized_places.borrow_mut().normalize(place))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>(),
    );

    PlaceDomain {
      domain,
      normalized_places,
    }
  }
}

impl IndexedDomain for PlaceDomain<'tcx> {
  type Index = PlaceIndex;
  type Value = Place<'tcx>;

  fn value(&self, index: Self::Index) -> &Self::Value {
    self.domain.value(index)
  }

  fn index(&self, value: &Self::Value) -> Self::Index {
    self
      .domain
      .index(&self.normalized_places.borrow_mut().normalize(*value))
  }

  fn len(&self) -> usize {
    self.domain.len()
  }

  fn iter_enumerated<'a>(&'a self) -> Enumerated<Self::Index, Iter<'a, Self::Value>> {
    self.domain.iter_enumerated()
  }
}

impl IndexedValue for Place<'tcx> {
  type Index = PlaceIndex;
  type Domain = PlaceDomain<'tcx>;
}

pub type PlaceSet<'tcx> = IndexSet<Place<'tcx>>;

// impl DebugWithContext<PlaceDomain<'tcx>> for PlaceSet {
//   fn fmt_with(&self, ctxt: &PlaceDomain<'tcx>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//     let format_place = |place: Place| {
//       let mut s = format!("{:?}", place.local);
//       for elem in place.projection.iter() {
//         s = match elem {
//           ProjectionElem::Deref => format!("(*{})", s),
//           ProjectionElem::Field(field, _) => format!("{}.{:?}", s, field.as_usize()),
//           ProjectionElem::Index(_) => format!("{}[]", s),
//           _ => format!("TODO({})", s),
//         };
//       }
//       s
//     };

//     write!(
//       f,
//       "{{{}}}",
//       self
//         .iter(ctxt)
//         .map(|place| format_place(place))
//         .collect::<Vec<_>>()
//         .join(", ")
//     )
//   }
// }

rustc_index::newtype_index! {
  pub struct LocationIndex {
      DEBUG_FORMAT = "l{}"
  }
}

impl IndexedValue for Location {
  type Index = LocationIndex;
}

pub type LocationSet = IndexSet<Location>;
pub type LocationDomain = <Location as IndexedValue>::Domain;

pub fn build_location_domain(body: &Body) -> Rc<LocationDomain> {
  let locations = body
    .basic_blocks()
    .iter_enumerated()
    .map(|(block, data)| {
      (0..data.statements.len() + 1).map(move |statement_index| Location {
        block,
        statement_index,
      })
    })
    .flatten()
    .collect::<Vec<_>>();
  Rc::new(LocationDomain::new(locations))
}
