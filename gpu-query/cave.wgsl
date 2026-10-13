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

struct type_17 {
    member: u32,
    member_1: u32,
}

var<private> global: vec3<u32>;
@group(0) @binding(0) 
var<storage> global_1: type_7;
@group(0) @binding(1) 
var<storage, read_write> global_2: type_10;

fn function_() {
    var local: array<type_13, 8>;
    var phi_119_: bool;
    var phi_121_: bool;
    var phi_122_: bool;
    var phi_145_: type_17;
    var phi_175_: u32;
    var phi_178_: type_13;
    var phi_692_: bool;
    var phi_702_: bool;
    var phi_712_: bool;
    var phi_722_: bool;
    var phi_732_: bool;
    var phi_742_: bool;
    var phi_752_: bool;
    var phi_762_: bool;
    var phi_785_: array<type_13, 8>;
    var phi_788_: array<type_13, 8>;
    var phi_789_: bool;
    var phi_809_: array<type_13, 8>;
    var phi_811_: array<type_13, 8>;
    var phi_256_: u32;
    var phi_258_: u32;
    var phi_259_: type_13;
    var phi_261_: bool;
    var phi_262_: bool;
    var phi_176_: u32;
    var phi_179_: type_13;
    var phi_265_: bool;
    var phi_267_: bool;
    var phi_268_: bool;
    var phi_1051_: bool;
    var phi_652_: bool;
    var phi_651_: bool;
    var phi_650_: bool;
    var phi_648_: type_13;
    var phi_277_: type_13;
    var phi_282_: type_13;
    var phi_940_: bool;
    var phi_941_: bool;
    var phi_949_: bool;
    var phi_950_: bool;
    var phi_951_: bool;
    var local_1: type_13;
    var local_2: type_13;

    switch bitcast<i32>(0u) {
        default: {
            let _e41 = global;
            let _e48 = global_1.member.member_2[0u];
            if (_e41.x < _e48) {
                let _e54 = global_1.member.member_2[1u];
                let _e55 = (_e41.y < _e54);
                if _e55 {
                    let _e60 = global_1.member.member_2[2u];
                    phi_119_ = (_e41.z < _e60);
                } else {
                    phi_119_ = bool();
                }
                let _e63 = phi_119_;
                phi_121_ = _e63;
                phi_122_ = select(true, false, _e55);
            } else {
                phi_121_ = bool();
                phi_122_ = true;
            }
            let _e66 = phi_121_;
            let _e68 = phi_122_;
            if select(_e66, false, _e68) {
                let _e74 = global_1.member.member_2[1u];
                phi_145_ = type_17(1u, (_e41.x + (_e48 * (_e41.y + (_e74 * _e41.z)))));
            } else {
                phi_145_ = type_17(0u, type_17().member_1);
            }
            let _e85 = phi_145_;
            if (bitcast<i32>(_e85.member) != 0i) {
                let _e94 = global_1.member.member;
                let _e98 = global_1.member.member_1[0u];
                let _e104 = global_1.member.member_1[1u];
                let _e110 = global_1.member.member_1[2u];
                phi_175_ = 19u;
                phi_178_ = type_13(false, true);
                loop {
                    let _e114 = phi_175_;
                    let _e116 = phi_178_;
                    local_1 = _e116;
                    local_2 = _e116;
                    let _e117 = (_e114 <= _e94);
                    if _e117 {
                        let _e118 = (_e114 <= 24u);
                        if _e118 {
                            let _e120 = ((_e94 - _e114) & 31u);
                            let _e122 = ((_e98 + bitcast<i32>(_e41.x)) >> bitcast<u32>(_e120));
                            let _e124 = ((_e104 + bitcast<i32>(_e41.y)) >> bitcast<u32>(_e120));
                            let _e126 = ((_e110 + bitcast<i32>(_e41.z)) >> bitcast<u32>(_e120));
                            let _e127 = (_e114 - 1u);
                            if (_e127 < 18u) {
                                phi_811_ = array<type_13, 8>(type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true), type_13(false, true));
                            } else {
                                if _e116.member_1 {
                                    let _e137 = (_e127 >= 24u);
                                    if _e137 {
                                        phi_785_ = array<type_13, 8>();
                                    } else {
                                        let _e139 = ((388248515u ^ _e127) + 2654435769u);
                                        let _e143 = ((_e139 ^ (_e139 >> bitcast<u32>(16i))) * 2246822507u);
                                        let _e147 = ((_e143 ^ (_e143 >> bitcast<u32>(13i))) * 3266489909u);
                                        let _e153 = (((_e147 ^ (_e147 >> bitcast<u32>(16i))) ^ bitcast<u32>((_e122 >> bitcast<u32>(1i)))) + 2654435769u);
                                        let _e157 = ((_e153 ^ (_e153 >> bitcast<u32>(16i))) * 2246822507u);
                                        let _e161 = ((_e157 ^ (_e157 >> bitcast<u32>(13i))) * 3266489909u);
                                        let _e167 = (((_e161 ^ (_e161 >> bitcast<u32>(16i))) ^ bitcast<u32>((_e124 >> bitcast<u32>(1i)))) + 2654435769u);
                                        let _e171 = ((_e167 ^ (_e167 >> bitcast<u32>(16i))) * 2246822507u);
                                        let _e175 = ((_e171 ^ (_e171 >> bitcast<u32>(13i))) * 3266489909u);
                                        let _e181 = (((_e175 ^ (_e175 >> bitcast<u32>(16i))) ^ bitcast<u32>((_e126 >> bitcast<u32>(1i)))) + 2654435769u);
                                        let _e185 = ((_e181 ^ (_e181 >> bitcast<u32>(16i))) * 2246822507u);
                                        let _e189 = ((_e185 ^ (_e185 >> bitcast<u32>(13i))) * 3266489909u);
                                        let _e192 = (_e189 ^ (_e189 >> bitcast<u32>(16i)));
                                        let _e193 = (_e192 < 1073741824u);
                                        let _e194 = (_e192 & 7u);
                                        let _e197 = ((_e192 >> bitcast<u32>(8i)) & 255u);
                                        if _e193 {
                                            phi_692_ = (_e194 == 0u);
                                        } else {
                                            phi_692_ = false;
                                        }
                                        let _e200 = phi_692_;
                                        if _e193 {
                                            phi_702_ = (_e194 == 1u);
                                        } else {
                                            phi_702_ = false;
                                        }
                                        let _e206 = phi_702_;
                                        if _e193 {
                                            phi_712_ = (_e194 == 2u);
                                        } else {
                                            phi_712_ = false;
                                        }
                                        let _e214 = phi_712_;
                                        if _e193 {
                                            phi_722_ = (_e194 == 3u);
                                        } else {
                                            phi_722_ = false;
                                        }
                                        let _e222 = phi_722_;
                                        if _e193 {
                                            phi_732_ = (_e194 == 4u);
                                        } else {
                                            phi_732_ = false;
                                        }
                                        let _e230 = phi_732_;
                                        if _e193 {
                                            phi_742_ = (_e194 == 5u);
                                        } else {
                                            phi_742_ = false;
                                        }
                                        let _e238 = phi_742_;
                                        if _e193 {
                                            phi_752_ = (_e194 == 6u);
                                        } else {
                                            phi_752_ = false;
                                        }
                                        let _e246 = phi_752_;
                                        if _e193 {
                                            phi_762_ = (_e194 == 7u);
                                        } else {
                                            phi_762_ = false;
                                        }
                                        let _e254 = phi_762_;
                                        phi_785_ = array<type_13, 8>(type_13((_e116.member != _e200), ((_e197 & 1u) == 1u)), type_13((_e116.member != _e206), (((_e197 >> bitcast<u32>(1u)) & 1u) == 1u)), type_13((_e116.member != _e214), (((_e197 >> bitcast<u32>(2u)) & 1u) == 1u)), type_13((_e116.member != _e222), (((_e197 >> bitcast<u32>(3u)) & 1u) == 1u)), type_13((_e116.member != _e230), (((_e197 >> bitcast<u32>(4u)) & 1u) == 1u)), type_13((_e116.member != _e238), (((_e197 >> bitcast<u32>(5u)) & 1u) == 1u)), type_13((_e116.member != _e246), (((_e197 >> bitcast<u32>(6u)) & 1u) == 1u)), type_13((_e116.member != _e254), (((_e197 >> bitcast<u32>(7u)) & 1u) == 1u)));
                                    }
                                    let _e270 = phi_785_;
                                    phi_788_ = _e270;
                                    phi_789_ = _e137;
                                } else {
                                    phi_788_ = array<type_13, 8>();
                                    phi_789_ = true;
                                }
                                let _e272 = phi_788_;
                                let _e274 = phi_789_;
                                if _e274 {
                                    let _e275 = type_13(_e116.member, false);
                                    phi_809_ = array<type_13, 8>(_e275, _e275, _e275, _e275, _e275, _e275, _e275, _e275);
                                } else {
                                    phi_809_ = _e272;
                                }
                                let _e278 = phi_809_;
                                phi_811_ = _e278;
                            }
                            let _e280 = phi_811_;
                            local = array<type_13, 8>(type_13(_e280[0].member, _e280[0].member_1), type_13(_e280[1].member, _e280[1].member_1), type_13(_e280[2].member, _e280[2].member_1), type_13(_e280[3].member, _e280[3].member_1), type_13(_e280[4].member, _e280[4].member_1), type_13(_e280[5].member, _e280[5].member_1), type_13(_e280[6].member, _e280[6].member_1), type_13(_e280[7].member, _e280[7].member_1));
                            let _e323 = bitcast<u32>((((_e122 & 1i) | ((_e124 & 1i) << bitcast<u32>(1i))) | ((_e126 & 1i) << bitcast<u32>(2i))));
                            if (_e323 < 8u) {
                            } else {
                                phi_1051_ = true;
                                phi_652_ = bool();
                                phi_651_ = bool();
                                phi_650_ = bool();
                                phi_648_ = type_13();
                                break;
                            }
                            let _e327 = local[_e323].member;
                            let _e330 = local[_e323].member_1;
                            if _e330 {
                                phi_256_ = (_e114 + 1u);
                            } else {
                                phi_256_ = u32();
                            }
                            let _e334 = phi_256_;
                            phi_258_ = _e334;
                            phi_259_ = type_13(_e327, _e330);
                            phi_261_ = _e330;
                            phi_262_ = select(true, false, _e330);
                        } else {
                            phi_258_ = u32();
                            phi_259_ = type_13();
                            phi_261_ = false;
                            phi_262_ = false;
                        }
                        let _e337 = phi_258_;
                        let _e339 = phi_259_;
                        let _e341 = phi_261_;
                        let _e343 = phi_262_;
                        phi_176_ = _e337;
                        phi_179_ = _e339;
                        phi_265_ = _e341;
                        phi_267_ = _e343;
                        phi_268_ = select(true, false, _e118);
                    } else {
                        phi_176_ = u32();
                        phi_179_ = type_13();
                        phi_265_ = false;
                        phi_267_ = false;
                        phi_268_ = false;
                    }
                    let _e346 = phi_176_;
                    let _e348 = phi_179_;
                    let _e350 = phi_265_;
                    let _e352 = phi_267_;
                    let _e354 = phi_268_;
                    continue;
                    continuing {
                        phi_175_ = _e346;
                        phi_178_ = _e348;
                        phi_1051_ = false;
                        phi_652_ = _e354;
                        phi_651_ = _e352;
                        phi_650_ = select(true, false, _e117);
                        phi_648_ = _e348;
                        break if !(_e350);
                    }
                }
                let _e358 = phi_1051_;
                let _e360 = phi_652_;
                let _e362 = phi_651_;
                let _e364 = phi_650_;
                let _e366 = phi_648_;
                if _e358 {
                    break;
                }
                let _e367 = select(_e362, false, _e364);
                if _e367 {
                    phi_277_ = _e366;
                } else {
                    let _e439 = local_1;
                    phi_277_ = _e439;
                }
                let _e370 = phi_277_;
                if select(select(_e360, false, _e364), false, _e367) {
                    let _e442 = local_2;
                    phi_282_ = _e442;
                } else {
                    phi_282_ = _e370;
                }
                let _e373 = phi_282_;
                if (_e94 >= 19u) {
                    if _e373.member_1 {
                        let _e377 = (_e94 >= 24u);
                        phi_940_ = _e377;
                        phi_941_ = _e377;
                    } else {
                        phi_940_ = true;
                        phi_941_ = bool();
                    }
                    let _e379 = phi_940_;
                    let _e381 = phi_941_;
                    phi_949_ = _e379;
                    phi_950_ = _e381;
                    phi_951_ = select(true, false, _e379);
                } else {
                    phi_949_ = bool();
                    phi_950_ = false;
                    phi_951_ = true;
                }
                let _e384 = phi_949_;
                let _e386 = phi_950_;
                let _e388 = phi_951_;
                if (_e85.member_1 < arrayLength((&global_2.member))) {
                } else {
                    break;
                }
                global_2.member[_e85.member_1] = ((select(0u, 1u, _e373.member) | (select(0u, 1u, select(_e384, _e386, _e388)) << bitcast<u32>(1i))) | (select(0u, 1u, select(_e373.member, false, _e388)) << bitcast<u32>(2i)));
            }
            break;
        }
    }
    return;
}

fn function_1() {
    var phi_579_: bool;
    var phi_581_: bool;
    var phi_582_: bool;
    var phi_605_: type_17;

    switch bitcast<i32>(0u) {
        default: {
            let _e40 = global;
            let _e47 = global_1.member.member_2[0u];
            if (_e40.x < _e47) {
                let _e53 = global_1.member.member_2[1u];
                let _e54 = (_e40.y < _e53);
                if _e54 {
                    let _e59 = global_1.member.member_2[2u];
                    phi_579_ = (_e40.z < _e59);
                } else {
                    phi_579_ = bool();
                }
                let _e62 = phi_579_;
                phi_581_ = _e62;
                phi_582_ = select(true, false, _e54);
            } else {
                phi_581_ = bool();
                phi_582_ = true;
            }
            let _e65 = phi_581_;
            let _e67 = phi_582_;
            if select(_e65, false, _e67) {
                let _e73 = global_1.member.member_2[1u];
                phi_605_ = type_17(1u, (_e40.x + (_e47 * (_e40.y + (_e73 * _e40.z)))));
            } else {
                phi_605_ = type_17(0u, type_17().member_1);
            }
            let _e84 = phi_605_;
            if (bitcast<i32>(_e84.member) != 0i) {
                let _e91 = global_1.member.member;
                let _e97 = global_1.member.member_1[0u];
                let _e103 = global_1.member.member_1[1u];
                let _e109 = global_1.member.member_1[2u];
                let _e113 = ((388248515u ^ _e91) + 2654435769u);
                let _e117 = ((_e113 ^ (_e113 >> bitcast<u32>(16i))) * 2246822507u);
                let _e121 = ((_e117 ^ (_e117 >> bitcast<u32>(13i))) * 3266489909u);
                let _e127 = (((_e121 ^ (_e121 >> bitcast<u32>(16i))) ^ bitcast<u32>((_e97 + bitcast<i32>(_e40.x)))) + 2654435769u);
                let _e131 = ((_e127 ^ (_e127 >> bitcast<u32>(16i))) * 2246822507u);
                let _e135 = ((_e131 ^ (_e131 >> bitcast<u32>(13i))) * 3266489909u);
                let _e141 = (((_e135 ^ (_e135 >> bitcast<u32>(16i))) ^ bitcast<u32>((_e103 + bitcast<i32>(_e40.y)))) + 2654435769u);
                let _e145 = ((_e141 ^ (_e141 >> bitcast<u32>(16i))) * 2246822507u);
                let _e149 = ((_e145 ^ (_e145 >> bitcast<u32>(13i))) * 3266489909u);
                let _e155 = (((_e149 ^ (_e149 >> bitcast<u32>(16i))) ^ bitcast<u32>((_e109 + bitcast<i32>(_e40.z)))) + 2654435769u);
                let _e159 = ((_e155 ^ (_e155 >> bitcast<u32>(16i))) * 2246822507u);
                let _e163 = ((_e159 ^ (_e159 >> bitcast<u32>(13i))) * 3266489909u);
                if (_e84.member_1 < arrayLength((&global_2.member))) {
                } else {
                    break;
                }
                global_2.member[_e84.member_1] = (_e163 ^ (_e163 >> bitcast<u32>(16i)));
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
