"""A checker for the suite's own JSON schemas (tests/fixtures/schemas/): the
keywords they use and no others. A keyword it does not know fails, so a
schema never says more than is checked. Not a general validator: the dev
shell has no `jsonschema` package, and these few lines need none.
"""

TYPES = {"object": dict, "array": list, "string": str, "boolean": bool, "null": type(None)}
# Keywords that say nothing a value can break.
NOTES = {"$schema", "$id", "title", "description"}


def is_type(value, name):
    """Whether `value` is of JSON type `name` (a bool is no number)."""
    if name in ("integer", "number"):
        whole = isinstance(value, int) or (name == "number" and isinstance(value, float))
        return whole and not isinstance(value, bool)
    return isinstance(value, TYPES[name])


def errors(value, schema, at="$"):
    """Every way `value` breaks `schema`, one `<path>: <why>` line each;
    empty when it holds."""
    out = []
    for key, want in schema.items():
        if key in NOTES:
            continue
        if key == "type":
            names = want if isinstance(want, list) else [want]
            if not any(is_type(value, n) for n in names):
                out.append(f"{at}: {value!r} is not {' or '.join(names)}")
        elif key == "const":
            if value != want:
                out.append(f"{at}: {value!r} is not {want!r}")
        elif key == "enum":
            if value not in want:
                out.append(f"{at}: {value!r} is none of {want!r}")
        elif key == "minimum":
            if is_type(value, "number") and value < want:
                out.append(f"{at}: {value!r} is under {want!r}")
        elif key == "required":
            if isinstance(value, dict):
                out += [f"{at}: no '{k}'" for k in want if k not in value]
        elif key == "properties":
            if isinstance(value, dict):
                for k, sub in want.items():
                    if k in value:
                        out += errors(value[k], sub, f"{at}.{k}")
        elif key == "additionalProperties":
            if isinstance(value, dict):
                for k in value.keys() - schema.get("properties", {}).keys():
                    if want is False:
                        out.append(f"{at}: '{k}' is not a key of the schema")
                    elif want is not True:
                        out += errors(value[k], want, f"{at}.{k}")
        elif key == "items":
            if isinstance(value, list):
                for i, v in enumerate(value):
                    out += errors(v, want, f"{at}[{i}]")
        elif key == "oneOf":
            held = sum(not errors(value, sub, at) for sub in want)
            if held != 1:
                out.append(f"{at}: holds {held} of the {len(want)} oneOf cases, not one")
        else:
            raise AssertionError(f"{at}: the schema keyword '{key}' is not one schema_check knows")
    return out
