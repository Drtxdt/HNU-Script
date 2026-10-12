use proptest::prelude::*;
use sql_killer_core::{algebra, database, exercises::tables_equal, model::*};
proptest! {
 #[test]
 fn selection_pushdown_preserves_bag_to_set_boundary(values in prop::collection::vec((0i32..5,prop::option::of(0i32..10)),0..20)){
  let c=database::memory("empty").unwrap();c.execute_batch("CREATE TABLE R(id INTEGER,v INTEGER);CREATE TABLE S(id INTEGER,name TEXT);INSERT INTO S VALUES(1,'a'),(1,'b'),(2,'c');").unwrap();
  for (id,v) in values {c.execute("INSERT INTO R VALUES(?1,?2)",(id,v)).unwrap();}
  let r=algebra::evaluate_algebra(&c,"project{id,name}(select{v>4}(join(R,S)))",true,&Budget::default()).unwrap();
  prop_assert!(r.equivalent);
  let sql=database::query(&c,"SELECT DISTINCT R.id,S.name FROM R JOIN S ON R.id=S.id WHERE R.v>4",&Budget::default()).unwrap();
  prop_assert!(tables_equal(&r.trace.result,&sql,false,true));
 }
}
