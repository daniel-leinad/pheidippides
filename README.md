# About

A simple messenger in a form of a web application.

This is mostly an experimental project for the purpose of self-education. This is still a work-in-progress.

Written in Rust, it consists of 2 executables: **server** for starting the server and **migrate** for managing migrations and uses **PostgreSQL** as backend.

A lot of things that were done would have been unnecessary/shouldn't have been done at all in "real" production code, like writing the handling of HTTP requests from scratch and implementing my own authentication system. Creating a custom deserializer for parsing form-data is probably overkill as well.

## Name

Pheidippides was a **messenger** from ancient Greece: https://en.wikipedia.org/wiki/Pheidippides

## Architecture

Application has a layered architecture:

```
Web server - responsible for handling requests and client rendering (pheidippides::routing)
↓
Database access - talks to the database: responsible for reading and modifying stored data (pheidippides::db)
↓
DBMS (Postgres) - database itself
```

(I intend to expand it in the future by adding an Application layer between the Web server layer and the Database access layer):

# Get started

Terminal commands should work on both Powershell 7 and bash 

## Build

You can build the application using cargo:

```
cargo build --release
cd target/release
```

App was written and tested for version **rustc 1.79.0-nightly (c9f8f3438 2024-03-27)**, built on windows 11

## Setup

App uses a connection url of a special format to connect to the database:

```
postgresql://[user[:password]@][host][:port][/dbname][?param1=value1&...]
```

1. Create an empty database in postgres
2. Run migrations against the database using the **migrate** executable (substitute all the necessary credentials):
```
./migrate postgresql://[user[:password]@][host][:port][/dbname][?param1=value1&...]
```
3. Start the server using the **server** executable
```
./server --host IP -p PORT --db postgresql://[user[:password]@][host][:port][/dbname][?param1=value1&...]
```
4. Use `--help` flag if questions arise


### Example

```
./migrate postgresql://postgres:postgres_password@localhost/pheidippides
./server --host localhost -p 8080 --db postgresql://postgres:postgres_password@localhost/pheidippides
```

## Usage

Open browser and enter host and port that you specified when starting the server (if you followed the example, that would be http://localhost:8080)

Client-side was tested on Google Chrome (123.0.6312.106) and Mozilla Firefox (124.0.1)

1. To start messaging you need to create at least 2 different accounts
2. After logging in, server remembers your session id so that you don't need to input login and password for the second time (unless you explicitly log out). Session ids are stored in memory, so you'll need to reenter credentials if you restart the server
3. After succesful login or signup, you'll be redirected to the `/chat` page. Initially it will be empty because you haven't sent or received any messages so far.
4. To start messaging, search for other accounts using the search form:
![](images/search_form_screenshot.png)
5. If all is good, your page should look something like this:
![](images/ui_screenshot.png)

# Issues

All discovered issues are documented in the GitHub issues
