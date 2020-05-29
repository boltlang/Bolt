
import { assert } from "chai"
import { AnyType, simplifyType, UnionType } from "../types"
import { createBoltIdentifier } from "../ast";
import { type } from "os";

describe('a function that merges two equivalent types', () => {

    it('can merge two any types', () =>{
        const type1 = new AnyType;
        type1.node = createBoltIdentifier('a');
        const type2 = new AnyType;
        type2.node = createBoltIdentifier('b');
        const types = new UnionType([type1 type2]);
        mergeTypes(types);
    })

})
