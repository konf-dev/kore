#!/usr/bin/env python3

import json
import os
import sys

TODO_FILE = 'todos.json'

def _load_todos():
    if not os.path.exists(TODO_FILE):
        return []
    with open(TODO_FILE, 'r') as f:
        try:
            return json.load(f)
        except json.JSONDecodeError:
            return []

def _save_todos(todos):
    with open(TODO_FILE, 'w') as f:
        json.dump(todos, f, indent=4)

def add_todo(description):
    todos = _load_todos()
    new_id = 1 if not todos else max(todo['id'] for todo in todos) + 1
    todos.append({'id': new_id, 'description': description, 'completed': False})
    _save_todos(todos)
    print(f'Added todo: "{description}" (ID: {new_id})')

def list_todos():
    todos = _load_todos()
    if not todos:
        print('No todos yet!')
        return

    print('--- Your Todos ---')
    for todo in todos:
        status = '[X]' if todo['completed'] else '[ ]'
        print(f'{status} {todo['id']}. {todo['description']}')
    print('------------------')

def complete_todo(todo_id):
    todos = _load_todos()
    found = False
    for todo in todos:
        if todo['id'] == todo_id:
            todo['completed'] = True
            found = True
            break
    if found:
        _save_todos(todos)
        print(f'Marked todo {todo_id} as complete.')
    else:
        print(f'Todo with ID {todo_id} not found.')

def delete_todo(todo_id):
    todos = _load_todos()
    initial_len = len(todos)
    todos = [todo for todo in todos if todo['id'] != todo_id]
    if len(todos) < initial_len:
        _save_todos(todos)
        print(f'Deleted todo {todo_id}.')
    else:
        print(f'Todo with ID {todo_id} not found.')

def main():
    if len(sys.argv) < 2:
        print('Usage: python todo.py <command> [args]')
        print('Commands:')
        print('  add <description>')
        print('  list')
        print('  complete <id>')
        print('  delete <id>')
        sys.exit(1)

    command = sys.argv[1]
    if command == 'add':
        if len(sys.argv) < 3:
            print('Usage: python todo.py add <description>')
            sys.exit(1)
        description = ' '.join(sys.argv[2:])
        add_todo(description)
    elif command == 'list':
        list_todos()
    elif command == 'complete':
        if len(sys.argv) < 3:
            print('Usage: python todo.py complete <id>')
            sys.exit(1)
        try:
            todo_id = int(sys.argv[2])
            complete_todo(todo_id)
        except ValueError:
            print('Error: ID must be an integer.')
            sys.exit(1)
    elif command == 'delete':
        if len(sys.argv) < 3:
            print('Usage: python todo.py delete <id>')
            sys.exit(1)
        try:
            todo_id = int(sys.argv[2])
            delete_todo(todo_id)
        except ValueError:
            print('Error: ID must be an integer.')
            sys.exit(1)
    else:
        print(f'Error: Unknown command "{command}"')
        sys.exit(1)

if __name__ == '__main__':
    main()
