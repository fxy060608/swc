//// [typeOfThisInStaticMembers5.ts]
import { _ as _class_call_check } from "@swc/helpers/_/_class_call_check";
var C = function C(foo) {
    "use strict";
    _class_call_check(this, C);
    this.foo = foo;
};
(function() {
    C.create = function() {
        return new C("yep");
    };
})();
