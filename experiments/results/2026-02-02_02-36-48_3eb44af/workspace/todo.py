import json


def load_todos():
    with open('todos.json', 'r') as file:
        return json.load(file)

def save_todos(todos):
    with open('todos.json', 'w') as file:
        json.dump(todos, file)

def add_todo(todo_text):
    todos = load_todos()
    todos.append({'text': todo_text, 'completed': False})
    save_todos(todos)

def list_todos():
    todos = load_todos()
    for index, todo in enumerate(todos):
        status = '[X]' if todo['completed'] else '[ ]'
        print(f'{index + 1}: {status} {todo["text"]}')

def complete_todo(index):
    todos = load_todos()
    if 0 <= index < len(todos):
        todos[index]['completed'] = True
        save_todos(todos)
    else:
        print('Invalid todo index')

def delete_todo(index):
    todos = load_todos()
    if 0 <= index < len(todos):
        del todos[index]
        save_todos(todos)
    else:
        print('Invalid todo index')