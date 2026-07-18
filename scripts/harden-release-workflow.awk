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
    if (install_count != 2 || !root_permission_hardened) {
        print "cargo-dist workflow structure was not fully recognized" > "/dev/stderr"
        exit 2
    }
}
