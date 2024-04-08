CREATE TABLE public.users
(
    user_id uuid NOT NULL,
    username character varying(150) COLLATE pg_catalog."default" NOT NULL,
    CONSTRAINT users_pkey PRIMARY KEY (user_id),
    CONSTRAINT users_username_key UNIQUE (username)
);

CREATE TABLE public.auth
(
    user_id uuid NOT NULL,
    phc_string character varying(250) COLLATE pg_catalog."default" NOT NULL,
    CONSTRAINT auth_pkey PRIMARY KEY (user_id),
    CONSTRAINT auth_user_id_fkey FOREIGN KEY (user_id)
        REFERENCES public.users (user_id) MATCH SIMPLE
        ON UPDATE NO ACTION
        ON DELETE NO ACTION
);

CREATE TABLE public.messages
(
    id uuid NOT NULL,
    "timestamp" timestamp with time zone NOT NULL,
    sender uuid NOT NULL,
    receiver uuid NOT NULL,
    message text COLLATE pg_catalog."default" NOT NULL,
    CONSTRAINT messages_pkey PRIMARY KEY (id),
    CONSTRAINT messages_from_fkey FOREIGN KEY (sender)
        REFERENCES public.users (user_id) MATCH SIMPLE
        ON UPDATE NO ACTION
        ON DELETE NO ACTION,
    CONSTRAINT messages_to_fkey FOREIGN KEY (receiver)
        REFERENCES public.users (user_id) MATCH SIMPLE
        ON UPDATE NO ACTION
        ON DELETE NO ACTION
);