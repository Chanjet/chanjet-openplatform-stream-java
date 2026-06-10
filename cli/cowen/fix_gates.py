import re

with open("scripts/quality_gate.sh", "r") as f:
    content = f.read()

# I will find the index of "# 7. Check unused dependencies"
# and the index of "# 9. Check Cargo Audit"
# and remove everything in between.

start_idx = content.find("# 7. Check unused dependencies (cargo machete)")
end_idx = content.find("# 9. Check Cargo Audit & Deny (Warn only for now)")

if start_idx != -1 and end_idx != -1:
    content = content[:start_idx] + content[end_idx:]

# And replace "# 9. Check Cargo Audit" back to "# 7. Check Cargo Audit" just in case.
content = content.replace("# 9. Check Cargo Audit & Deny (Warn only for now)", "# 7. Check Cargo Audit & Deny (Warn only for now)")

with open("scripts/quality_gate.sh", "w") as f:
    f.write(content)

