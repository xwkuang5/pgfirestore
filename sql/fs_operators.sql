CREATE OPERATOR -> (
    LEFTARG = fsvalue,
    RIGHTARG = text,
    FUNCTION = fs_map_get
);

CREATE OPERATOR #< ( 
LEFTARG = fsvalue,
RIGHTARG = fsvalue,
FUNCTION = fs_lt
);

CREATE OPERATOR #> ( 
LEFTARG = fsvalue,
RIGHTARG = fsvalue,
FUNCTION = fs_gt
);

CREATE OPERATOR #<= ( 
LEFTARG = fsvalue,
RIGHTARG = fsvalue,
FUNCTION = fs_le
);

CREATE OPERATOR #>= ( 
LEFTARG = fsvalue,
RIGHTARG = fsvalue,
FUNCTION = fs_ge
);

CREATE OPERATOR #!= ( 
LEFTARG = fsvalue,
RIGHTARG = fsvalue,
FUNCTION = fs_neq
);

CREATE OPERATOR #= ( 
LEFTARG = fsvalue,
RIGHTARG = fsvalue,
FUNCTION = fs_eq
);