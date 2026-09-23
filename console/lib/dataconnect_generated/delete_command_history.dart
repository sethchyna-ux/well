part of 'generated.dart';

class DeleteCommandHistoryVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  DeleteCommandHistoryVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<DeleteCommandHistoryData> dataDeserializer = (dynamic json)  => DeleteCommandHistoryData.fromJson(jsonDecode(json));
  Serializer<DeleteCommandHistoryVariables> varsSerializer = (DeleteCommandHistoryVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<DeleteCommandHistoryData, DeleteCommandHistoryVariables>> execute() {
    return ref().execute();
  }

  MutationRef<DeleteCommandHistoryData, DeleteCommandHistoryVariables> ref() {
    DeleteCommandHistoryVariables vars= DeleteCommandHistoryVariables(id: id,);
    return _dataConnect.mutation("DeleteCommandHistory", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class DeleteCommandHistoryCommandHistoryDelete {
  final String id;
  DeleteCommandHistoryCommandHistoryDelete.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteCommandHistoryCommandHistoryDelete otherTyped = other as DeleteCommandHistoryCommandHistoryDelete;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteCommandHistoryCommandHistoryDelete({
    required this.id,
  });
}

@immutable
class DeleteCommandHistoryData {
  final DeleteCommandHistoryCommandHistoryDelete? commandHistory_delete;
  DeleteCommandHistoryData.fromJson(dynamic json):
  
  commandHistory_delete = json['commandHistory_delete'] == null ? null : DeleteCommandHistoryCommandHistoryDelete.fromJson(json['commandHistory_delete']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteCommandHistoryData otherTyped = other as DeleteCommandHistoryData;
    return commandHistory_delete == otherTyped.commandHistory_delete;
    
  }
  @override
  int get hashCode => commandHistory_delete.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (commandHistory_delete != null) {
      json['commandHistory_delete'] = commandHistory_delete!.toJson();
    }
    return json;
  }

  DeleteCommandHistoryData({
    this.commandHistory_delete,
  });
}

@immutable
class DeleteCommandHistoryVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  DeleteCommandHistoryVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteCommandHistoryVariables otherTyped = other as DeleteCommandHistoryVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteCommandHistoryVariables({
    required this.id,
  });
}

