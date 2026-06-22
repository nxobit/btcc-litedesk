@compute @workgroup_size(64)
fn compute_jacobian(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= config.num_keys) { return; }

    var base_pub: JacobianPoint;
    base_pub.x = unpack_bigint(config.base_x);
    base_pub.y = unpack_bigint(config.base_y);
    base_pub.z = fe_one();

    let point_i = table_rw[idx];
    let p_res = jac_add_affine(base_pub, point_i);
    jacobian_points[idx] = p_res;
}
