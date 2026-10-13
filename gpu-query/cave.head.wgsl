struct type_6 {
    member: u32,
    member_1: array<i32, 3>,
    member_2: array<u32, 3>,
}

struct type_7 {
    member: type_6,
}

struct type_10 {
    member: array<u32>,
}

struct type_13 {
    member: bool,
    member_1: bool,
}

struct type_21 {
    member: u32,
    member_1: u32,
    member_2: bool,
}

struct type_22 {
    member: u32,
    member_1: u32,
}

struct type_25 {
    member: u64,
    member_1: u64,
}

var<private> global: vec3<u32>;
@group(0) @binding(0) 
var<storage> global_1: type_7;
@group(0) @binding(1) 
var<storage, read_write> global_2: type_10;

fn function_() {
    var local: array<type_13, 8>;
    var local_1: array<i64, 3>;
    var phi_954_: bool;
    var phi_180_: type_21;
    var phi_183_: type_13;
    var phi_195_: bool;
    var phi_220_: type_21;
    var phi_181_: type_21;
    var phi_223_: type_22;
    var phi_284_: u32;
    var phi_287_: u64;
    var phi_285_: u32;
    var phi_288_: u64;
    var phi_948_: bool;
    var local_2: u64;
    var local_3: u64;
    var phi_314_: type_25;
    var local_4: u64;
    var phi_339_: bool;
    var phi_360_: bool;
    var phi_381_: bool;
    var phi_402_: bool;
    var phi_423_: bool;
    var phi_444_: bool;
    var phi_465_: bool;
    var phi_486_: bool;
    var phi_962_: bool;
    var phi_509_: array<type_13, 8>;
    var phi_961_: bool;
    var phi_510_: array<type_13, 8>;
    var phi_511_: bool;
    var phi_531_: array<type_13, 8>;
    var phi_959_: bool;
    var phi_532_: array<type_13, 8>;
    var phi_957_: bool;
    var phi_184_: type_13;
    var phi_584_: bool;
    var phi_585_: bool;
    var phi_956_: bool;
    var phi_798_: bool;
    var phi_796_: type_13;
    var phi_589_: type_13;
    var phi_884_: u32;
    var phi_885_: u32;
    var phi_886_: bool;
    var phi_892_: u32;
    var phi_893_: u32;
    var phi_894_: bool;
    var local_5: type_13;

    switch bitcast<i32>(0u) {
        default: {
            let _e51 = global;
            let _e58 = global_1.member.member_2[0u];
            let _e63 = global_1.member.member_2[1u];
            let _e68 = (_e51.x + (_e58 * (_e51.y + (_e63 * _e51.z))));
            if (_e51.x < _e58) {
                if (_e51.y < _e63) {
                    let _e74 = global_1.member.member_2[2u];
                    if (_e51.z < _e74) {
                        let _e79 = global_1.member.member_1[0u];
                        let _e86 = global_1.member.member_1[1u];
                        let _e93 = global_1.member.member_1[2u];
                        let _e99 = global_1.member.member;
                        phi_954_ = false;
                        phi_180_ = type_21(19u, select(_e99, 24u, (24u < _e99)), false);
                        phi_183_ = type_13(false, true);
                        loop {
                            let _e104 = phi_954_;
                            let _e106 = phi_180_;
                            let _e108 = phi_183_;
                            local_5 = _e108;
                            if _e106.member_2 {
                                phi_195_ = true;
                            } else {
                                phi_195_ = !((_e106.member <= _e106.member_1));
                            }
                            let _e115 = phi_195_;
                            if _e115 {
                                phi_181_ = _e106;
                                phi_223_ = type_22(0u, type_22().member_1);
                            } else {
                                if (_e106.member < _e106.member_1) {
                                    phi_220_ = type_21((_e106.member + 1u), _e106.member_1, _e106.member_2);
                                } else {
                                    phi_220_ = type_21(_e106.member, _e106.member_1, true);
                                }
                                let _e129 = phi_220_;
                                phi_181_ = _e129;
                                phi_223_ = type_22(1u, _e106.member);
                            }
                            let _e135 = phi_181_;
                            let _e137 = phi_223_;
                            if (bitcast<i32>(_e137.member) != 0i) {
                                let _e144 = i64(((_e99 - _e137.member_1) & 63u));
                                let _e146 = ((i64(_e79) + i64(_e51.x)) >> u32(_e144));
                                let _e148 = ((i64(_e86) + i64(_e51.y)) >> u32(_e144));
                                let _e150 = ((i64(_e93) + i64(_e51.z)) >> u32(_e144));
                                let _e153 = (_e137.member_1 - 1u);
                                if (_e153 < 18u) {
                                    phi_959_ = _e104;
                                    phi_532_ = array<type_13, 8>(type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true));
                                } else {
                                    if _e108.member_1 {
                                        let _e161 = (_e153 >= 24u);
                                        if _e161 {
                                            phi_962_ = _e104;
                                            phi_509_ = array<type_13, 8>();
                                        } else {
                                            local_1 = array<i64, 3>((_e146 >> 1u), (_e148 >> 1u), (_e150 >> 1u));
                                            let _e164 = ((15042267673225134081lu ^ 3397255169lu) + 11400714819323198485lu);
                                            let _e168 = ((_e164 ^ (_e164 >> 30u)) * 13787848793156543929lu);
                                            let _e172 = ((_e168 ^ (_e168 >> 27u)) * 10723151780598845931lu);
                                            let _e178 = (((_e172 ^ (_e172 >> 31u)) ^ u64(_e153)) + 11400714819323198485lu);
                                            let _e182 = ((_e178 ^ (_e178 >> 30u)) * 13787848793156543929lu);
                                            let _e186 = ((_e182 ^ (_e182 >> 27u)) * 10723151780598845931lu);
                                            phi_284_ = 0u;
                                            phi_287_ = (_e186 ^ (_e186 >> 31u));
                                            loop {
                                                let _e191 = phi_284_;
                                                let _e193 = phi_287_;
                                                local_2 = _e193;
                                                local_3 = _e193;
                                                local_4 = _e193;
                                                let _e194 = (_e191 < 3u);
                                                if _e194 {
                                                    if _e194 {
                                                    } else {
                                                        phi_948_ = true;
                                                        break;
                                                    }
                                                    let _e196 = local_1[_e191];
                                                    let _e199 = ((_e193 ^ bitcast<u64>(_e196)) + 11400714819323198485lu);
                                                    let _e203 = ((_e199 ^ (_e199 >> 30u)) * 13787848793156543929lu);
                                                    let _e207 = ((_e203 ^ (_e203 >> 27u)) * 10723151780598845931lu);
                                                    phi_285_ = (_e191 + 1u);
                                                    phi_288_ = (_e207 ^ (_e207 >> 31u));
                                                } else {
                                                    phi_285_ = u32();
                                                    phi_288_ = u64();
                                                }
                                                let _e213 = phi_285_;
                                                let _e215 = phi_288_;
                                                continue;
                                                continuing {
                                                    phi_284_ = _e213;
                                                    phi_287_ = _e215;
                                                    phi_948_ = _e104;
                                                    break if !(_e194);
                                                }
                                            }
                                            let _e218 = phi_948_;
                                            phi_956_ = _e218;
                                            phi_798_ = bool();
                                            phi_796_ = type_13();
                                            if _e218 {
                                                break;
                                            }
                                            let _e220 = local_2;
                                            let _e224 = local_3;
                                            if (u32(_e220) < 1073741824u) {
                                                phi_314_ = type_25(1lu, ((_e224 >> 32u) & 7lu));
                                            } else {
                                                phi_314_ = type_25(0lu, type_25().member_1);
                                            }
                                            let _e233 = phi_314_;
                                            let _e237 = local_4;
                                            let _e241 = u32(((_e237 >> 40u) & 255lu));
                                            let _e243 = (i32(_e233.member) != 0i);
                                            if _e243 {
                                                phi_339_ = (_e233.member_1 == 0lu);
                                            } else {
                                                phi_339_ = false;
                                            }
                                            let _e246 = phi_339_;
                                            if _e243 {
                                                phi_360_ = (_e233.member_1 == 1lu);
                                            } else {
                                                phi_360_ = false;
                                            }
                                            let _e256 = phi_360_;
                                            if _e243 {
                                                phi_381_ = (_e233.member_1 == 2lu);
                                            } else {
                                                phi_381_ = false;
                                            }
                                            let _e266 = phi_381_;
                                            if _e243 {
                                                phi_402_ = (_e233.member_1 == 3lu);
                                            } else {
                                                phi_402_ = false;
                                            }
                                            let _e276 = phi_402_;
                                            if _e243 {
                                                phi_423_ = (_e233.member_1 == 4lu);
                                            } else {
                                                phi_423_ = false;
                                            }
                                            let _e286 = phi_423_;
                                            if _e243 {
                                                phi_444_ = (_e233.member_1 == 5lu);
                                            } else {
                                                phi_444_ = false;
                                            }
                                            let _e296 = phi_444_;
                                            if _e243 {
                                                phi_465_ = (_e233.member_1 == 6lu);
                                            } else {
                                                phi_465_ = false;
                                            }
                                            let _e306 = phi_465_;
                                            if _e243 {
                                                phi_486_ = (_e233.member_1 == 7lu);
                                            } else {
                                                phi_486_ = false;
                                            }
                                            let _e316 = phi_486_;
                                            phi_962_ = _e218;
                                            phi_509_ = array<type_13, 8>(type_13((_e108.member != _e246), (((_e241 >> bitcast<u32>(u32((0lu & 31lu)))) & 1u) == 1u)), type_13((_e108.member != _e256), (((_e241 >> bitcast<u32>(u32((1lu & 31lu)))) & 1u) == 1u)), type_13((_e108.member != _e266), (((_e241 >> bitcast<u32>(u32((2lu & 31lu)))) & 1u) == 1u)), type_13((_e108.member != _e276), (((_e241 >> bitcast<u32>(u32((3lu & 31lu)))) & 1u) == 1u)), type_13((_e108.member != _e286), (((_e241 >> bitcast<u32>(u32((4lu & 31lu)))) & 1u) == 1u)), type_13((_e108.member != _e296), (((_e241 >> bitcast<u32>(u32((5lu & 31lu)))) & 1u) == 1u)), type_13((_e108.member != _e306), (((_e241 >> bitcast<u32>(u32((6lu & 31lu)))) & 1u) == 1u)), type_13((_e108.member != _e316), (((_e241 >> bitcast<u32>(u32((7lu & 31lu)))) & 1u) == 1u)));
                                        }
                                        let _e334 = phi_962_;
                                        let _e336 = phi_509_;
                                        phi_961_ = _e334;
                                        phi_510_ = _e336;
                                        phi_511_ = _e161;
                                    } else {
                                        phi_961_ = _e104;
                                        phi_510_ = array<type_13, 8>();
                                        phi_511_ = true;
                                    }
                                    let _e338 = phi_961_;
                                    let _e340 = phi_510_;
                                    let _e342 = phi_511_;
                                    if _e342 {
                                        let _e343 = type_13(_e108.member, false);
                                        phi_531_ = array<type_13, 8>(_e343, _e343, _e343, _e343, _e343, _e343, _e343, _e343);
                                    } else {
                                        phi_531_ = _e340;
                                    }
                                    let _e346 = phi_531_;
                                    phi_959_ = _e338;
                                    phi_532_ = _e346;
                                }
                                let _e348 = phi_959_;
                                let _e350 = phi_532_;
                                local = array<type_13, 8>(type_13(_e350[0].member, _e350[0].member_1), type_13(_e350[1].member, _e350[1].member_1), type_13(_e350[2].member, _e350[2].member_1), type_13(_e350[3].member, _e350[3].member_1), type_13(_e350[4].member, _e350[4].member_1), type_13(_e350[5].member, _e350[5].member_1), type_13(_e350[6].member, _e350[6].member_1), type_13(_e350[7].member, _e350[7].member_1));
                                let _e394 = u32(bitcast<u64>((((_e146 & 1li) | ((_e148 & 1li) << 1u)) | ((_e150 & 1li) << 2u))));
                                if (_e394 < 8u) {
                                } else {
                                    phi_956_ = true;
                                    phi_798_ = bool();
                                    phi_796_ = type_13();
                                    break;
                                }
                                let _e398 = local[_e394].member;
                                let _e401 = local[_e394].member_1;
                                phi_957_ = _e348;
                                phi_184_ = type_13(_e398, _e401);
                                phi_584_ = _e401;
                                phi_585_ = select(true, false, _e401);
                            } else {
                                phi_957_ = _e104;
                                phi_184_ = type_13();
                                phi_584_ = false;
                                phi_585_ = false;
                            }
                            let _e405 = phi_957_;
                            let _e407 = phi_184_;
                            let _e409 = phi_584_;
                            let _e411 = phi_585_;
                            continue;
                            continuing {
                                phi_954_ = _e405;
                                phi_180_ = _e135;
                                phi_183_ = _e407;
                                phi_956_ = _e405;
                                phi_798_ = _e411;
                                phi_796_ = _e407;
                                break if !(_e409);
                            }
                        }
                        let _e414 = phi_956_;
                        let _e416 = phi_798_;
                        let _e418 = phi_796_;
                        if _e414 {
                            break;
                        }
                        if _e416 {
                            phi_589_ = _e418;
                        } else {
                            let _e493 = local_5;
                            phi_589_ = _e493;
                        }
                        let _e420 = phi_589_;
                        if (_e99 >= 19u) {
                            if _e420.member_1 {
                                let _e425 = (_e99 >= 24u);
                                let _e426 = select(0u, 1u, _e425);
                                phi_884_ = _e426;
                                phi_885_ = _e426;
                                phi_886_ = _e425;
                            } else {
                                phi_884_ = 1u;
                                phi_885_ = u32();
                                phi_886_ = true;
                            }
                            let _e428 = phi_884_;
                            let _e430 = phi_885_;
                            let _e432 = phi_886_;
                            phi_892_ = _e428;
                            phi_893_ = _e430;
                            phi_894_ = select(true, false, _e432);
                        } else {
                            phi_892_ = u32();
                            phi_893_ = 0u;
                            phi_894_ = true;
                        }
                        let _e435 = phi_892_;
                        let _e437 = phi_893_;
                        let _e439 = phi_894_;
                        if (_e68 < arrayLength((&global_2.member))) {
                        } else {
                            break;
                        }
                        global_2.member[_e68] = (select(0u, 1u, _e420.member) | ((select(_e435, _e437, _e439) | (select(0u, 1u, select(_e420.member, false, _e439)) << bitcast<u32>(1i))) << bitcast<u32>(1i)));
                    }
                }
            }
            break;
        }
    }
    return;
}

fn function_1() {
    var local_6: array<i64, 3>;
    var phi_749_: u32;
    var phi_752_: u64;
    var phi_750_: u32;
    var phi_753_: u64;
    var phi_980_: bool;
    var local_7: u64;
    var local_8: u64;

    switch bitcast<i32>(0u) {
        default: {
            let _e50 = global;
            let _e52 = arrayLength((&global_2.member));
            let _e57 = global_1.member.member_2[0u];
            let _e62 = global_1.member.member_2[1u];
            if (_e50.x < _e57) {
                if (_e50.y < _e62) {
                    let _e73 = global_1.member.member_2[2u];
                    if (_e50.z < _e73) {
                        let _e78 = global_1.member.member_1[0u];
                        let _e85 = global_1.member.member_1[1u];
                        let _e92 = global_1.member.member_1[2u];
                        let _e98 = global_1.member.member;
                        local_6 = array<i64, 3>((i64(_e78) + i64(_e50.x)), (i64(_e85) + i64(_e50.y)), (i64(_e92) + i64(_e50.z)));
                        let _e101 = ((15042267673225134081lu ^ 3397255169lu) + 11400714819323198485lu);
                        let _e105 = ((_e101 ^ (_e101 >> 30u)) * 13787848793156543929lu);
                        let _e109 = ((_e105 ^ (_e105 >> 27u)) * 10723151780598845931lu);
                        let _e115 = (((_e109 ^ (_e109 >> 31u)) ^ u64(_e98)) + 11400714819323198485lu);
                        let _e119 = ((_e115 ^ (_e115 >> 30u)) * 13787848793156543929lu);
                        let _e123 = ((_e119 ^ (_e119 >> 27u)) * 10723151780598845931lu);
                        phi_749_ = 0u;
                        phi_752_ = (_e123 ^ (_e123 >> 31u));
                        loop {
                            let _e128 = phi_749_;
                            let _e130 = phi_752_;
                            local_7 = _e130;
                            local_8 = _e130;
                            let _e131 = (_e128 < 3u);
                            if _e131 {
                                if _e131 {
                                } else {
                                    phi_980_ = true;
                                    break;
                                }
                                let _e133 = local_6[_e128];
                                let _e136 = ((_e130 ^ bitcast<u64>(_e133)) + 11400714819323198485lu);
                                let _e140 = ((_e136 ^ (_e136 >> 30u)) * 13787848793156543929lu);
                                let _e144 = ((_e140 ^ (_e140 >> 27u)) * 10723151780598845931lu);
                                phi_750_ = (_e128 + 1u);
                                phi_753_ = (_e144 ^ (_e144 >> 31u));
                            } else {
                                phi_750_ = u32();
                                phi_753_ = u64();
                            }
                            let _e150 = phi_750_;
                            let _e152 = phi_753_;
                            continue;
                            continuing {
                                phi_749_ = _e150;
                                phi_752_ = _e152;
                                phi_980_ = false;
                                break if !(_e131);
                            }
                        }
                        let _e155 = phi_980_;
                        if _e155 {
                            break;
                        }
                        let _e157 = local_7;
                        let _e160 = local_8;
                        let _e164 = (2u * (_e50.x + (_e57 * (_e50.y + (_e62 * _e50.z)))));
                        if (_e164 < _e52) {
                        } else {
                            break;
                        }
                        global_2.member[_e164] = u32((_e123 ^ (_e123 >> 31u)));
                        let _e168 = (_e164 + 1u);
                        if (_e168 < _e52) {
                        } else {
                            break;
                        }
                        global_2.member[_e168] = u32(((_e123 ^ (_e123 >> 31u)) >> 32u));
                    }
                }
            }
            break;
        }
    }
    return;
}

@compute @workgroup_size(4, 4, 4) 
fn main_cs(@builtin(global_invocation_id) param: vec3<u32>) {
    global = param;
    function_();
}

@compute @workgroup_size(4, 4, 4) 
fn seed_cs(@builtin(global_invocation_id) param_1: vec3<u32>) {
    global = param_1;
    function_1();
}
