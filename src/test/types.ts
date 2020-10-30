
import test from "ava";

import { AnyType, IntersectType, PrimType } from "../checker"
import { createRef } from "../util";

test('an intersection with an any-type should remove that any-type', t => {
  const t1 = new IntersectType([
    createRef(new AnyType()),
    createRef(new PrimType('@int')),
  ])
  
});
