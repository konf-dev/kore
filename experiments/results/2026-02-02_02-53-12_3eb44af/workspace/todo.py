
import json
import os
import sys

TODO_FILE = 'todos.json'

def load_todos():
    if os.path.exists(TODO_FILE):
        with open(TODO_FILE, 'r') as f:
            return json.load(f)
    return []

def save_todos(todos):
    with open(TODO_FILE, 'w') as f:
        json.dump(todos, f, indent=4)

def add_todo(task):
    todos = load_todos()
    todos.append({'task': task, 'completed': False})
    save_todos(todos)
    print(f'Added todo: "{task}"')

def list_todos():
    todos = load_todos()
    if not todos:
        print('No todos yet!')
        return
    for i, todo in enumerate(todos):
        status = '[x]' if todo['completed'] else '[ ]'
        print(f'{i}. {status} {todo["task"]}')

def complete_todo(index):
    todos = load_todos()
    if 0 <= index < len(todos):
        if not todos[index]['completed']:
            todos[index]['completed'] = True
            save_todos(todos)
            print(f'Marked todo {index} as complete: "{todos[index]["task"]}"')
        else:
            print(f'Todo {index} is already complete.')
    else:
        print(f'Invalid todo index: {index}')

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print('Usage: python todo.py <command> [args]')
        print('Commands: add <task>, list, complete <index>')
    else:
        command = sys.argv[1]
        if command == 'add':
            if len(sys.argv) > 2:
                task = ' '.join(sys.argv[2:])
                add_todo(task)
            else:
                print('Usage: python todo.py add <task>')
        elif command == 'list':
            list_todos()
        elif command == 'complete':
            if len(sys.argv) > 2:
                try:
                    index = int(sys.argv[2])
                    complete_todo(index)
                except ValueError:
                    print('Error: Index must be an integer.')
            else:
                print('Usage: python todo.py complete <index>')
        else:
            print(f'Unknown command: {command}')
