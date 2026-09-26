function emit(path, line) {
    while ((getline line < path) > 0) {
        print line
    }
    close(path)
}

BEGIN {
    install_count = 0
    skip_install = 0
    root_permission_hardened = 0
    in_host = 0
    dist_command_count = 0
    in_global_build = 0
    local_record_count = 0
    global_record_count = 0
    build_verify_count = 0
    build_env_count = 0
    build_upload_count = 0
    release_cleanup_count = 0
}

skip_install {
    if ($0 ~ /^      - (name|id|uses|run):/) {
        skip_install = 0
    } else {
        next
    }
}

$0 == "  \"contents\": \"write\"" && !root_permission_hardened {
    print "  \"contents\": \"read\""
    root_permission_hardened = 1
    next
}

$0 == "      - name: Install dist" {
    install_count++
    if (install_count == 1) {
        emit(plan_fragment)
    } else if (install_count == 2) {
        emit(build_fragment)
    } else {
        print "unexpected additional cargo-dist installer" > "/dev/stderr"
        exit 2
    }
    skip_install = 1
    next
}

$0 == "  host:" {
    in_host = 1
    print
    print "    # Only the job that creates the GitHub Release receives write access."
    print "    # Build jobs execute downloaded tooling with a read-only token."
    print "    permissions:"
    print "      \"contents\": \"write\""
    next
}

$0 ~ /^[[:space:]]+dist / && $0 ~ / --output-format=json/ {
    sub(/ --output-format=json/, " --allow-dirty --output-format=json")
    dist_command_count++
}

$0 == "  build-global-artifacts:" {
    in_global_build = 1
}

$0 == "  host:" {
    in_global_build = 0
}

$0 ~ /^      BUILD_MANIFEST_NAME: / {
    print
    if (in_global_build) {
        print "      BUILD_ENV_NAME: target/distrib/global-build-environment.json"
    } else {
        print "      BUILD_ENV_NAME: target/distrib/${{ join(matrix.targets, '-') }}-build-environment.json"
    }
    build_env_count++
    next
}

$0 == "      - name: Build artifacts" {
    emit(local_record_fragment)
    local_record_count++
}

$0 == "      - id: cargo-dist" && !in_global_build && local_record_count == 1 && build_verify_count == 0 {
    emit(build_verify_fragment)
    build_verify_count++
}

$0 == "      - id: cargo-dist" && in_global_build {
    emit(global_record_fragment)
    global_record_count++
}

$0 == "      - name: \"Upload artifacts\"" && in_global_build && global_record_count == 1 {
    emit(build_verify_fragment)
    build_verify_count++
}

$0 == "            ${{ env.BUILD_MANIFEST_NAME }}" {
    print
    print "            ${{ env.BUILD_ENV_NAME }}"
    build_upload_count++
    next
}

in_host && $0 == "          rm -f artifacts/*-dist-manifest.json" {
    print
    print "          rm -f artifacts/*-build-environment.json"
    release_cleanup_count++
    next
}

$0 ~ /^  [a-zA-Z0-9_-]+:$/ && $0 != "  host:" {
    in_host = 0
}

in_host && $0 == "    runs-on: \"ubuntu-22.04\"" && previous == "      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}" {
    print
    print "    environment: release"
    previous = $0
    next
}

{
    print
    previous = $0
}

END {
    if (install_count != 2 || !root_permission_hardened || dist_command_count != 4 ||
        local_record_count != 1 || global_record_count != 1 || build_verify_count != 2 ||
        build_env_count != 2 || build_upload_count != 2 || release_cleanup_count != 1) {
        print "cargo-dist workflow structure was not fully recognized" > "/dev/stderr"
        exit 2
    }
}
