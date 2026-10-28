#!/system/bin/sh
# Android Optimizer: full (speed) compilation of the apps a person opens (icons),
# plus System UI and the home screen. Other system components are left alone:
# there are hundreds of them, they rarely run, and compiling all of them would take hours.

home=$(cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.HOME | tail -n 1 | cut -d/ -f1)
apps=$(cmd package query-activities --brief -a android.intent.action.MAIN -c android.intent.category.LAUNCHER | sed -n 's#^ *\([A-Za-z0-9_.]*\)/.*#\1#p' | sort -u)

set -- com.android.systemui $home $apps
total=$#
i=0
for p in "$@"; do
  i=$((i + 1))
  if cmd package compile -m speed "$p" 2>&1 | grep -q Success; then
    echo "$i/$total OK $p"
  else
    echo "$i/$total -- $p"
  fi
done
